//! GPU pipeline cache for @flow shaders
//!
//! Compiles FlowGraph → WGSL → wgpu pipeline on first use, caches by flow name.
//! Handles both fragment (render) and compute (simulation) pipelines.
//! Uniform buffers are updated per-frame with runtime data (time, pointer, etc.).

use std::collections::HashMap;
use std::sync::Arc;

use blinc_core::{FlowGraph, FlowInputSource, FlowTarget, FlowType};

use crate::flow_codegen::flow_to_wgsl;

// ==========================================================================
// Uniform Data
// ==========================================================================

/// Runtime data written to the flow uniform buffer each frame.
///
/// Layout matches the `FlowUniforms` struct emitted by `flow_codegen.rs`.
/// Fields are tightly packed as vec4-aligned f32 arrays for GPU upload.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FlowUniformData {
    /// Viewport size in pixels
    pub viewport_size: [f32; 2],
    /// Elapsed time in seconds
    pub time: f32,
    /// Frame index (monotonic counter)
    pub frame_index: f32,
    /// Element bounds: [x, y, width, height] in pixels
    pub element_bounds: [f32; 4],
    /// Pointer position (from pointer query system)
    pub pointer: [f32; 2],
    /// Corner radius in pixels (for rounded-rect clipping)
    pub corner_radius: f32,
    /// Padding to maintain 16-byte alignment
    pub _padding: f32,
}

impl Default for FlowUniformData {
    fn default() -> Self {
        Self {
            viewport_size: [0.0; 2],
            time: 0.0,
            frame_index: 0.0,
            element_bounds: [0.0; 4],
            pointer: [0.0; 2],
            corner_radius: 0.0,
            _padding: 0.0,
        }
    }
}

// ==========================================================================
// Cached Pipeline
// ==========================================================================

/// A compiled and cached flow pipeline ready for rendering
struct CachedFlowPipeline {
    /// The compiled WGSL source (kept for debugging)
    #[allow(dead_code)]
    wgsl_source: String,
    /// Render pipeline (for fragment flows)
    render_pipeline: Option<wgpu::RenderPipeline>,
    /// Compute pipeline (for compute flows)
    compute_pipeline: Option<wgpu::ComputePipeline>,
    /// Bind group layout (shared between render and compute)
    bind_group_layout: wgpu::BindGroupLayout,
    /// Uniform buffer (updated per-frame)
    uniform_buffer: wgpu::Buffer,
    /// Storage buffers for compute I/O, keyed by name
    storage_buffers: HashMap<String, wgpu::Buffer>,
    /// The flow target type
    target: FlowTarget,
    /// Size of the uniform struct in bytes (base + dynamic CSS/env fields)
    uniform_size: u64,
}

// ==========================================================================
// Pipeline Cache
// ==========================================================================

/// Cache of compiled @flow GPU pipelines.
///
/// Created once per renderer and persists across frames.
/// Pipelines are compiled lazily on first use.
pub struct FlowPipelineCache {
    device: Arc<wgpu::Device>,
    texture_format: wgpu::TextureFormat,
    pipelines: HashMap<String, CachedFlowPipeline>,
}

impl FlowPipelineCache {
    /// Create a new empty pipeline cache
    pub fn new(device: Arc<wgpu::Device>, texture_format: wgpu::TextureFormat) -> Self {
        Self {
            device,
            texture_format,
            pipelines: HashMap::new(),
        }
    }

    /// Check if a flow pipeline is already compiled
    pub fn contains(&self, flow_name: &str) -> bool {
        self.pipelines.contains_key(flow_name)
    }

    /// Compile a FlowGraph into a GPU pipeline and cache it.
    ///
    /// Returns Ok(()) on success, or an error string on compilation failure.
    /// If the pipeline already exists, this is a no-op.
    pub fn compile(&mut self, graph: &FlowGraph) -> Result<(), String> {
        if self.pipelines.contains_key(&graph.name) {
            return Ok(());
        }

        // Generate WGSL
        let wgsl = flow_to_wgsl(graph).map_err(|e| format!("codegen error: {}", e))?;

        // Create shader module
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&format!("Flow Shader: {}", graph.name)),
                source: wgpu::ShaderSource::Wgsl(wgsl.clone().into()),
            });

        // Count dynamic uniform fields (CSS properties + env vars)
        let dynamic_field_count = graph
            .inputs
            .iter()
            .filter(|i| {
                matches!(
                    i.source,
                    FlowInputSource::CssProperty(_) | FlowInputSource::EnvVar(_)
                )
            })
            .count();

        // Compute uniform buffer size: base FlowUniformData + dynamic fields (each f32-aligned)
        // Dynamic fields are appended after the base struct.
        // Each dynamic field is padded to 4 bytes (f32) for simplicity.
        let base_size = std::mem::size_of::<FlowUniformData>() as u64;
        let dynamic_size = (dynamic_field_count * 4) as u64; // Each field is at most f32
                                                             // Round up to 16-byte alignment for uniform buffer requirements
        let uniform_size = ((base_size + dynamic_size + 15) / 16) * 16;
        // Minimum 256 bytes to satisfy minUniformBufferOffsetAlignment on some GPUs
        let uniform_size = uniform_size.max(256);

        // Create uniform buffer
        let uniform_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("Flow Uniforms: {}", graph.name)),
            size: uniform_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Collect storage buffer bindings
        let mut storage_buffers = HashMap::new();
        let mut storage_entries = Vec::new();
        let mut binding_index = 1u32; // binding(0) is uniforms

        for input in &graph.inputs {
            if let FlowInputSource::Buffer { name, ty } = &input.source {
                if !storage_buffers.contains_key(name) {
                    let elem_size = match ty {
                        FlowType::Float => 4,
                        FlowType::Vec2 => 8,
                        FlowType::Vec3 => 12,
                        FlowType::Vec4 => 16,
                    };
                    // Default buffer size: 64K elements
                    let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(&format!("Flow Buffer: {}", name)),
                        size: elem_size * 65536,
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    storage_buffers.insert(name.clone(), buf);
                    storage_entries.push((binding_index, name.clone(), false));
                    binding_index += 1;
                }
            }
        }

        // Check for buffer outputs that need read_write access
        for output in &graph.outputs {
            if let blinc_core::FlowOutputTarget::Buffer { name } = &output.target {
                if !storage_buffers.contains_key(name) {
                    let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(&format!("Flow Buffer: {}", name)),
                        size: 16 * 65536, // vec4 * 64K
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    storage_buffers.insert(name.clone(), buf);
                    storage_entries.push((binding_index, name.clone(), true));
                    binding_index += 1;
                } else {
                    // Promote existing buffer to read_write
                    for entry in &mut storage_entries {
                        if entry.1 == *name {
                            entry.2 = true;
                        }
                    }
                }
            }
        }

        // Build bind group layout entries
        let mut layout_entries = vec![
            // Binding 0: Uniforms
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: match graph.target {
                    FlowTarget::Fragment => {
                        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT
                    }
                    FlowTarget::Compute => wgpu::ShaderStages::COMPUTE,
                },
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ];

        // Add storage buffer entries
        for (binding, _name, is_rw) in &storage_entries {
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding: *binding,
                visibility: match graph.target {
                    FlowTarget::Fragment => wgpu::ShaderStages::FRAGMENT,
                    FlowTarget::Compute => wgpu::ShaderStages::COMPUTE,
                },
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: !*is_rw },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        }

        let bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some(&format!("Flow Bind Group Layout: {}", graph.name)),
                    entries: &layout_entries,
                });

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&format!("Flow Pipeline Layout: {}", graph.name)),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let mut render_pipeline = None;
        let mut compute_pipeline = None;

        match graph.target {
            FlowTarget::Fragment => {
                let blend_state = wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                };

                let rp = self
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(&format!("Flow Render Pipeline: {}", graph.name)),
                        layout: Some(&pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: Some("vs_main"),
                            buffers: &[],
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: Some("fs_main"),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: self.texture_format,
                                blend: Some(blend_state),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                        }),
                        primitive: wgpu::PrimitiveState {
                            topology: wgpu::PrimitiveTopology::TriangleList,
                            strip_index_format: None,
                            front_face: wgpu::FrontFace::Ccw,
                            cull_mode: None,
                            polygon_mode: wgpu::PolygonMode::Fill,
                            unclipped_depth: false,
                            conservative: false,
                        },
                        depth_stencil: None,
                        multisample: wgpu::MultisampleState::default(),
                        multiview: None,
                        cache: None,
                    });

                render_pipeline = Some(rp);
            }
            FlowTarget::Compute => {
                let cp = self
                    .device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some(&format!("Flow Compute Pipeline: {}", graph.name)),
                        layout: Some(&pipeline_layout),
                        module: &shader,
                        entry_point: Some("cs_main"),
                        compilation_options: Default::default(),
                        cache: None,
                    });

                compute_pipeline = Some(cp);
            }
        }

        self.pipelines.insert(
            graph.name.clone(),
            CachedFlowPipeline {
                wgsl_source: wgsl,
                render_pipeline,
                compute_pipeline,
                bind_group_layout,
                uniform_buffer,
                storage_buffers,
                target: graph.target,
                uniform_size,
            },
        );

        Ok(())
    }

    /// Invalidate (remove) a cached pipeline, forcing recompilation on next use.
    pub fn invalidate(&mut self, flow_name: &str) {
        self.pipelines.remove(flow_name);
    }

    /// Invalidate all cached pipelines.
    pub fn invalidate_all(&mut self) {
        self.pipelines.clear();
    }

    /// Update the uniform buffer for a flow and return the bind group for rendering.
    ///
    /// Returns None if the flow is not compiled.
    pub fn prepare_render(
        &self,
        queue: &wgpu::Queue,
        flow_name: &str,
        uniforms: &FlowUniformData,
    ) -> Option<wgpu::BindGroup> {
        let cached = self.pipelines.get(flow_name)?;

        // Write base uniforms
        queue.write_buffer(&cached.uniform_buffer, 0, bytemuck::bytes_of(uniforms));

        // Build bind group with current resources
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: cached.uniform_buffer.as_entire_binding(),
        }];

        // Add storage buffer entries (binding indices start at 1)
        let mut binding = 1u32;
        // We need to iterate in the same order as the layout was created.
        // For simplicity, iterate storage_buffers in sorted key order.
        let mut buf_names: Vec<&String> = cached.storage_buffers.keys().collect();
        buf_names.sort();
        for name in buf_names {
            if let Some(buf) = cached.storage_buffers.get(name.as_str()) {
                entries.push(wgpu::BindGroupEntry {
                    binding,
                    resource: buf.as_entire_binding(),
                });
                binding += 1;
            }
        }

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("Flow Bind Group: {}", flow_name)),
            layout: &cached.bind_group_layout,
            entries: &entries,
        });

        Some(bind_group)
    }

    /// Render a fragment flow into a render pass.
    ///
    /// The caller must have already called `prepare_render()` to get the bind group.
    /// Draws a fullscreen quad (6 vertices from the flow's vertex shader).
    pub fn render_fragment<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        flow_name: &str,
        bind_group: &'a wgpu::BindGroup,
    ) -> bool {
        let cached = match self.pipelines.get(flow_name) {
            Some(c) => c,
            None => return false,
        };

        let pipeline = match &cached.render_pipeline {
            Some(p) => p,
            None => return false,
        };

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..6, 0..1); // Fullscreen quad: 6 vertices (2 triangles)
        true
    }

    /// Dispatch a compute flow.
    ///
    /// The caller must have already called `prepare_render()` to get the bind group.
    pub fn dispatch_compute<'a>(
        &'a self,
        pass: &mut wgpu::ComputePass<'a>,
        flow_name: &str,
        bind_group: &'a wgpu::BindGroup,
        workgroup_count: u32,
    ) -> bool {
        let cached = match self.pipelines.get(flow_name) {
            Some(c) => c,
            None => return false,
        };

        let pipeline = match &cached.compute_pipeline {
            Some(p) => p,
            None => return false,
        };

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count, 1, 1);
        true
    }

    /// Get the target type of a compiled flow
    pub fn flow_target(&self, flow_name: &str) -> Option<FlowTarget> {
        self.pipelines.get(flow_name).map(|c| c.target)
    }

    /// Get the number of compiled pipelines
    pub fn len(&self) -> usize {
        self.pipelines.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }
}

// ==========================================================================
// Tests
// ==========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_uniform_data_size() {
        // Ensure the uniform struct is properly aligned for GPU upload
        let size = std::mem::size_of::<FlowUniformData>();
        assert_eq!(size, 48); // 12 f32s * 4 bytes = 48 bytes
        assert_eq!(size % 4, 0); // Must be f32-aligned
    }

    #[test]
    fn test_flow_uniform_data_default() {
        let data = FlowUniformData::default();
        assert_eq!(data.time, 0.0);
        assert_eq!(data.frame_index, 0.0);
        assert_eq!(data.viewport_size, [0.0; 2]);
    }
}

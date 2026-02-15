//! DAG-based flow language for declarative shader composition
//!
//! The `@flow` system provides a CSS-embedded dataflow graph language that
//! compiles to WGSL shaders. It supports both fragment (rendering) and
//! compute (simulation) targets, with compile-time cycle detection and
//! type inference.
//!
//! # Example
//!
//! ```css
//! @flow ripple-effect {
//!   target: fragment;
//!   input uv;
//!   input time;
//!   node dist = distance(uv, vec2(0.5, 0.5));
//!   node wave = sin(dist * 20.0 - time * 4.0);
//!   output color = vec4(wave, wave, wave, 1.0);
//! }
//! ```

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

// ===========================================================================
// Flow Graph — the top-level DAG structure
// ===========================================================================

/// A parsed `@flow` block — the DAG intermediate representation
#[derive(Clone, Debug)]
pub struct FlowGraph {
    /// Name of the flow (referenced by CSS `flow: name`)
    pub name: String,
    /// Target pipeline: fragment shader or compute shader
    pub target: FlowTarget,
    /// Workgroup size for compute flows (default: 64)
    pub workgroup_size: Option<u32>,
    /// Input declarations (builtins, buffers, CSS properties)
    pub inputs: Vec<FlowInput>,
    /// Processing nodes (the DAG vertices)
    pub nodes: Vec<FlowNode>,
    /// Output declarations (what the flow produces)
    pub outputs: Vec<FlowOutput>,
    /// Cached topological order (node indices, set after validation)
    pub topo_order: Vec<usize>,
}

/// Target pipeline for a flow
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowTarget {
    /// Generates fragment shader code for visual output
    Fragment,
    /// Generates compute shader code for simulation
    Compute,
}

impl FlowGraph {
    /// Create a new empty flow graph
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            target: FlowTarget::Fragment,
            workgroup_size: None,
            inputs: Vec::new(),
            nodes: Vec::new(),
            outputs: Vec::new(),
            topo_order: Vec::new(),
        }
    }

    /// Look up a name (input or node) and return its index in the combined namespace
    fn resolve_name(&self, name: &str) -> Option<NameResolution> {
        // Check inputs first
        for (i, input) in self.inputs.iter().enumerate() {
            if input.name == name {
                return Some(NameResolution::Input(i));
            }
        }
        // Then check nodes
        for (i, node) in self.nodes.iter().enumerate() {
            if node.name == name {
                return Some(NameResolution::Node(i));
            }
        }
        None
    }
}

#[derive(Debug)]
enum NameResolution {
    Input(usize),
    Node(usize),
}

// ===========================================================================
// Inputs
// ===========================================================================

/// An input declaration in a flow graph
#[derive(Clone, Debug)]
pub struct FlowInput {
    /// Name of the input (used as reference in node expressions)
    pub name: String,
    /// Where this input comes from
    pub source: FlowInputSource,
    /// Inferred or declared type
    pub ty: Option<FlowType>,
}

/// Source of a flow input value
#[derive(Clone, Debug, PartialEq)]
pub enum FlowInputSource {
    /// Built-in variable (uv, time, resolution, etc.)
    Builtin(BuiltinVar),
    /// Read from a named GPU buffer
    Buffer { name: String, ty: FlowType },
    /// Bound from a CSS property value
    CssProperty(String),
    /// From an environment variable (e.g., pointer-x)
    EnvVar(String),
    /// Unresolved — will be determined during validation
    Auto,
}

/// Built-in variables available to flows
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinVar {
    /// Element UV coordinates [0,1]
    Uv,
    /// Frame time in seconds
    Time,
    /// Element size in pixels
    Resolution,
    /// Current element's SDF value at the sample point
    Sdf,
    /// Frame index (integer)
    FrameIndex,
    /// Pointer position (vec2, from pointer query system)
    Pointer,
}

impl BuiltinVar {
    /// Parse a builtin variable name
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "uv" => Some(Self::Uv),
            "time" => Some(Self::Time),
            "resolution" => Some(Self::Resolution),
            "sdf" => Some(Self::Sdf),
            "frame-index" | "frame_index" => Some(Self::FrameIndex),
            "pointer" => Some(Self::Pointer),
            _ => None,
        }
    }

    /// Get the output type of this builtin
    pub fn output_type(&self) -> FlowType {
        match self {
            Self::Uv | Self::Resolution | Self::Pointer => FlowType::Vec2,
            Self::Time | Self::Sdf | Self::FrameIndex => FlowType::Float,
        }
    }
}

// ===========================================================================
// Nodes — DAG vertices
// ===========================================================================

/// A processing node in the flow graph
#[derive(Clone, Debug)]
pub struct FlowNode {
    /// Name of this node (used for references)
    pub name: String,
    /// The computation expression
    pub expr: FlowExpr,
    /// Inferred type (set during validation)
    pub inferred_type: Option<FlowType>,
}

// ===========================================================================
// Expressions — the computation graph
// ===========================================================================

/// A flow expression — the computation at each node
#[derive(Clone, Debug, PartialEq)]
pub enum FlowExpr {
    // Literals
    /// Scalar float literal
    Float(f32),
    /// 2-component vector constructor
    Vec2(Box<FlowExpr>, Box<FlowExpr>),
    /// 3-component vector constructor
    Vec3(Box<FlowExpr>, Box<FlowExpr>, Box<FlowExpr>),
    /// 4-component vector constructor
    Vec4(Box<FlowExpr>, Box<FlowExpr>, Box<FlowExpr>, Box<FlowExpr>),
    /// Color literal (#hex → RGBA)
    Color(f32, f32, f32, f32),

    // References (edges in the DAG)
    /// Reference to an input or earlier node by name
    Ref(String),

    // Arithmetic
    /// Addition: `a + b`
    Add(Box<FlowExpr>, Box<FlowExpr>),
    /// Subtraction: `a - b`
    Sub(Box<FlowExpr>, Box<FlowExpr>),
    /// Multiplication: `a * b`
    Mul(Box<FlowExpr>, Box<FlowExpr>),
    /// Division: `a / b`
    Div(Box<FlowExpr>, Box<FlowExpr>),
    /// Negation: `-a`
    Neg(Box<FlowExpr>),

    // Swizzle access
    /// Component access: `expr.xy`, `expr.rgb`, etc.
    Swizzle(Box<FlowExpr>, String),

    // Function calls (validated set — NOT arbitrary)
    /// Built-in function call
    Call { func: FlowFunc, args: Vec<FlowExpr> },
}

impl FlowExpr {
    /// Collect all `Ref` names in this expression tree
    pub fn collect_refs(&self, refs: &mut HashSet<String>) {
        match self {
            Self::Ref(name) => {
                refs.insert(name.clone());
            }
            Self::Float(_) | Self::Color(_, _, _, _) => {}
            Self::Vec2(a, b)
            | Self::Add(a, b)
            | Self::Sub(a, b)
            | Self::Mul(a, b)
            | Self::Div(a, b) => {
                a.collect_refs(refs);
                b.collect_refs(refs);
            }
            Self::Vec3(a, b, c) => {
                a.collect_refs(refs);
                b.collect_refs(refs);
                c.collect_refs(refs);
            }
            Self::Vec4(a, b, c, d) => {
                a.collect_refs(refs);
                b.collect_refs(refs);
                c.collect_refs(refs);
                d.collect_refs(refs);
            }
            Self::Neg(a) | Self::Swizzle(a, _) => a.collect_refs(refs),
            Self::Call { args, .. } => {
                for arg in args {
                    arg.collect_refs(refs);
                }
            }
        }
    }
}

// ===========================================================================
// Built-in functions
// ===========================================================================

/// Built-in function identifiers for the flow language
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowFunc {
    // ── Math (scalar) ──
    Sin,
    Cos,
    Tan,
    Abs,
    Floor,
    Ceil,
    Fract,
    Sqrt,
    Pow,
    Exp,
    Log,
    Sign,
    Mod,

    // ── Math (comparative) ──
    Min,
    Max,
    Clamp,
    Mix,
    Smoothstep,
    Step,

    // ── Vector ──
    Length,
    Distance,
    Dot,
    Cross,
    Normalize,
    Reflect,

    // ── SDF primitives ──
    SdfBox,
    SdfCircle,
    SdfEllipse,
    SdfRoundRect,

    // ── SDF combinators ──
    SdfUnion,
    SdfIntersect,
    SdfSubtract,
    SdfSmoothUnion,
    SdfSmoothIntersect,
    SdfSmoothSubtract,

    /// Current element's SDF at sample point
    Sdf,

    // ── Texture / buffer ──
    /// Sobel filter (compute normal from heightfield)
    Sobel,
    /// Read from a named buffer
    BufferRead,

    // ── Lighting ──
    Phong,
    BlinnPhong,

    // ── Noise ──
    Perlin,
    Simplex,
    Worley,
    /// Fractal Brownian Motion (layered noise)
    Fbm,

    // ── Simulation helpers ──
    SpringEval,
    WaveStep,
    FluidStep,
}

impl FlowFunc {
    /// Parse a function name to a FlowFunc
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "sin" => Some(Self::Sin),
            "cos" => Some(Self::Cos),
            "tan" => Some(Self::Tan),
            "abs" => Some(Self::Abs),
            "floor" => Some(Self::Floor),
            "ceil" => Some(Self::Ceil),
            "fract" => Some(Self::Fract),
            "sqrt" => Some(Self::Sqrt),
            "pow" => Some(Self::Pow),
            "exp" => Some(Self::Exp),
            "log" => Some(Self::Log),
            "sign" => Some(Self::Sign),
            "mod" => Some(Self::Mod),

            "min" => Some(Self::Min),
            "max" => Some(Self::Max),
            "clamp" => Some(Self::Clamp),
            "mix" => Some(Self::Mix),
            "smoothstep" => Some(Self::Smoothstep),
            "step" => Some(Self::Step),

            "length" => Some(Self::Length),
            "distance" => Some(Self::Distance),
            "dot" => Some(Self::Dot),
            "cross" => Some(Self::Cross),
            "normalize" => Some(Self::Normalize),
            "reflect" => Some(Self::Reflect),

            "sdf_box" | "sdf-box" => Some(Self::SdfBox),
            "sdf_circle" | "sdf-circle" => Some(Self::SdfCircle),
            "sdf_ellipse" | "sdf-ellipse" => Some(Self::SdfEllipse),
            "sdf_round_rect" | "sdf-round-rect" => Some(Self::SdfRoundRect),
            "sdf_union" | "sdf-union" => Some(Self::SdfUnion),
            "sdf_intersect" | "sdf-intersect" => Some(Self::SdfIntersect),
            "sdf_subtract" | "sdf-subtract" => Some(Self::SdfSubtract),
            "sdf_smooth_union" | "sdf-smooth-union" => Some(Self::SdfSmoothUnion),
            "sdf_smooth_intersect" | "sdf-smooth-intersect" => Some(Self::SdfSmoothIntersect),
            "sdf_smooth_subtract" | "sdf-smooth-subtract" => Some(Self::SdfSmoothSubtract),
            "sdf" => Some(Self::Sdf),

            "sobel" => Some(Self::Sobel),
            "buffer_read" | "buffer-read" => Some(Self::BufferRead),

            "phong" => Some(Self::Phong),
            "blinn_phong" | "blinn-phong" => Some(Self::BlinnPhong),

            "perlin" => Some(Self::Perlin),
            "simplex" => Some(Self::Simplex),
            "worley" => Some(Self::Worley),
            "fbm" => Some(Self::Fbm),

            "spring_eval" | "spring-eval" => Some(Self::SpringEval),
            "wave_step" | "wave-step" => Some(Self::WaveStep),
            "fluid_step" | "fluid-step" => Some(Self::FluidStep),

            _ => None,
        }
    }

    /// Get the expected argument count for this function
    pub fn arg_count(&self) -> (usize, usize) {
        // (min_args, max_args)
        match self {
            // 1-arg scalar functions
            Self::Sin
            | Self::Cos
            | Self::Tan
            | Self::Abs
            | Self::Floor
            | Self::Ceil
            | Self::Fract
            | Self::Sqrt
            | Self::Exp
            | Self::Log
            | Self::Sign
            | Self::Length
            | Self::Normalize
            | Self::Sdf => (1, 1),

            // 2-arg functions
            Self::Pow
            | Self::Mod
            | Self::Min
            | Self::Max
            | Self::Step
            | Self::Distance
            | Self::Dot
            | Self::Reflect
            | Self::Sobel
            | Self::SdfCircle
            | Self::SdfEllipse
            | Self::SdfUnion
            | Self::SdfIntersect
            | Self::SdfSubtract => (2, 2),

            // 3-arg functions
            Self::Clamp
            | Self::Mix
            | Self::Smoothstep
            | Self::Cross
            | Self::SdfBox
            | Self::SdfRoundRect
            | Self::SdfSmoothUnion
            | Self::SdfSmoothIntersect
            | Self::SdfSmoothSubtract
            | Self::Phong
            | Self::BlinnPhong
            | Self::Perlin
            | Self::Simplex
            | Self::Worley
            | Self::Fbm => (2, 4),

            // Variable-arg functions
            Self::BufferRead => (1, 3),
            Self::SpringEval | Self::WaveStep | Self::FluidStep => (2, 5),
        }
    }

    /// Get the return type given argument types
    pub fn return_type(&self, arg_types: &[FlowType]) -> Option<FlowType> {
        match self {
            // Scalar → Scalar
            Self::Sin
            | Self::Cos
            | Self::Tan
            | Self::Abs
            | Self::Floor
            | Self::Ceil
            | Self::Fract
            | Self::Sqrt
            | Self::Exp
            | Self::Log
            | Self::Sign
            | Self::Length
            | Self::Distance
            | Self::Dot
            | Self::Sdf
            | Self::SdfBox
            | Self::SdfCircle
            | Self::SdfEllipse
            | Self::SdfRoundRect
            | Self::SdfUnion
            | Self::SdfIntersect
            | Self::SdfSubtract
            | Self::SdfSmoothUnion
            | Self::SdfSmoothIntersect
            | Self::SdfSmoothSubtract
            | Self::Step => Some(FlowType::Float),

            // Preserve input type
            Self::Pow
            | Self::Mod
            | Self::Min
            | Self::Max
            | Self::Clamp
            | Self::Mix
            | Self::Smoothstep
            | Self::Normalize
            | Self::Reflect => arg_types.first().cloned().or(Some(FlowType::Float)),

            // Vector operations
            Self::Cross => Some(FlowType::Vec3),

            // Noise → Float
            Self::Perlin | Self::Simplex | Self::Worley | Self::Fbm => Some(FlowType::Float),

            // Sobel → Vec3 (normal)
            Self::Sobel => Some(FlowType::Vec3),

            // Lighting → Vec4 (color)
            Self::Phong | Self::BlinnPhong => Some(FlowType::Vec4),

            // Simulation → depends
            Self::SpringEval | Self::WaveStep | Self::FluidStep => {
                arg_types.first().cloned().or(Some(FlowType::Float))
            }

            Self::BufferRead => Some(FlowType::Vec4),
        }
    }
}

// ===========================================================================
// Types
// ===========================================================================

/// Flow data types
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowType {
    Float,
    Vec2,
    Vec3,
    Vec4,
}

impl FlowType {
    /// Number of components
    pub fn components(&self) -> usize {
        match self {
            Self::Float => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Vec4 => 4,
        }
    }

    /// Can this type be broadcast with another?
    pub fn broadcast_with(&self, other: &Self) -> Option<Self> {
        if self == other {
            Some(*self)
        } else if *self == Self::Float {
            Some(*other)
        } else if *other == Self::Float {
            Some(*self)
        } else {
            None // incompatible types (e.g., Vec2 + Vec3)
        }
    }
}

impl fmt::Display for FlowType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Float => write!(f, "float"),
            Self::Vec2 => write!(f, "vec2"),
            Self::Vec3 => write!(f, "vec3"),
            Self::Vec4 => write!(f, "vec4"),
        }
    }
}

// ===========================================================================
// Outputs
// ===========================================================================

/// An output declaration in a flow graph
#[derive(Clone, Debug)]
pub struct FlowOutput {
    /// Name of the output
    pub name: String,
    /// Where this output goes
    pub target: FlowOutputTarget,
    /// Optional expression (if None, uses node with same name)
    pub expr: Option<FlowExpr>,
}

/// Target for a flow output
#[derive(Clone, Debug, PartialEq)]
pub enum FlowOutputTarget {
    /// Fragment shader output color
    Color,
    /// Fragment shader output alpha
    Alpha,
    /// Fragment SDF displacement
    Displacement,
    /// Write to a named compute buffer
    Buffer { name: String },
    /// Expose as a CSS variable (GPU → CPU readback)
    CssVar(String),
}

// ===========================================================================
// Validation Errors
// ===========================================================================

/// Errors from flow graph validation
#[derive(Clone, Debug)]
pub enum FlowError {
    /// Cycle detected in the DAG
    CycleDetected {
        /// Names of nodes involved in the cycle
        nodes: Vec<String>,
    },
    /// Reference to an undefined name
    UndefinedReference {
        /// The node containing the reference
        in_node: String,
        /// The undefined name
        name: String,
    },
    /// Type mismatch in an operation
    TypeMismatch {
        /// The node where the error occurred
        in_node: String,
        /// Description of the mismatch
        message: String,
    },
    /// Wrong number of arguments to a function
    ArgCountMismatch {
        /// The node where the error occurred
        in_node: String,
        /// The function name
        func: String,
        /// Expected argument count range
        expected: (usize, usize),
        /// Actual argument count
        got: usize,
    },
    /// Missing required output for the target type
    MissingOutput {
        /// What's missing
        message: String,
    },
    /// Duplicate node name
    DuplicateName {
        /// The duplicated name
        name: String,
    },
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CycleDetected { nodes } => {
                write!(f, "cycle detected in flow graph: {}", nodes.join(" → "))
            }
            Self::UndefinedReference { in_node, name } => {
                write!(f, "in node '{}': undefined reference '{}'", in_node, name)
            }
            Self::TypeMismatch { in_node, message } => {
                write!(f, "in node '{}': type mismatch: {}", in_node, message)
            }
            Self::ArgCountMismatch {
                in_node,
                func,
                expected,
                got,
            } => {
                write!(
                    f,
                    "in node '{}': function '{}' expects {}-{} args, got {}",
                    in_node, func, expected.0, expected.1, got
                )
            }
            Self::MissingOutput { message } => write!(f, "missing output: {}", message),
            Self::DuplicateName { name } => {
                write!(f, "duplicate name '{}' in flow graph", name)
            }
        }
    }
}

// ===========================================================================
// Validation — Cycle Detection & Type Inference
// ===========================================================================

impl FlowGraph {
    /// Validate the flow graph: detect cycles, check types, resolve references.
    ///
    /// On success, populates `topo_order` with node indices in dependency order.
    /// On failure, returns all errors found.
    pub fn validate(&mut self) -> Result<(), Vec<FlowError>> {
        let mut errors = Vec::new();

        // 1. Check for duplicate names
        self.check_duplicates(&mut errors);

        // 2. Check all references resolve
        self.check_references(&mut errors);

        // 3. Topological sort (cycle detection via Kahn's algorithm)
        match self.topological_sort() {
            Ok(order) => self.topo_order = order,
            Err(cycle_nodes) => {
                errors.push(FlowError::CycleDetected { nodes: cycle_nodes });
            }
        }

        // 4. Type inference (only if no cycles)
        if errors
            .iter()
            .all(|e| !matches!(e, FlowError::CycleDetected { .. }))
        {
            self.infer_types(&mut errors);
        }

        // 5. Validate function argument counts
        self.check_function_args(&mut errors);

        // 6. Validate outputs match target
        self.check_outputs(&mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn check_duplicates(&self, errors: &mut Vec<FlowError>) {
        let mut names = HashSet::new();
        for input in &self.inputs {
            if !names.insert(&input.name) {
                errors.push(FlowError::DuplicateName {
                    name: input.name.clone(),
                });
            }
        }
        for node in &self.nodes {
            if !names.insert(&node.name) {
                errors.push(FlowError::DuplicateName {
                    name: node.name.clone(),
                });
            }
        }
    }

    fn check_references(&self, errors: &mut Vec<FlowError>) {
        let mut all_names: HashSet<String> = HashSet::new();
        for input in &self.inputs {
            all_names.insert(input.name.clone());
        }
        for node in &self.nodes {
            all_names.insert(node.name.clone());
        }

        for node in &self.nodes {
            let mut refs = HashSet::new();
            node.expr.collect_refs(&mut refs);
            for r in &refs {
                if !all_names.contains(r) {
                    errors.push(FlowError::UndefinedReference {
                        in_node: node.name.clone(),
                        name: r.clone(),
                    });
                }
            }
        }

        // Check output expressions too
        for output in &self.outputs {
            if let Some(expr) = &output.expr {
                let mut refs = HashSet::new();
                expr.collect_refs(&mut refs);
                for r in &refs {
                    if !all_names.contains(r) {
                        errors.push(FlowError::UndefinedReference {
                            in_node: format!("output:{}", output.name),
                            name: r.clone(),
                        });
                    }
                }
            } else {
                // Bare output name — must reference an existing name
                if !all_names.contains(&output.name) {
                    errors.push(FlowError::UndefinedReference {
                        in_node: "output".to_string(),
                        name: output.name.clone(),
                    });
                }
            }
        }
    }

    /// Topological sort using Kahn's algorithm.
    /// Returns Ok(order) with node indices in dependency order,
    /// or Err(cycle_nodes) with names of nodes in the cycle.
    fn topological_sort(&self) -> Result<Vec<usize>, Vec<String>> {
        let n = self.nodes.len();
        if n == 0 {
            return Ok(Vec::new());
        }

        // Build name → node index map
        let mut node_index: HashMap<&str, usize> = HashMap::new();
        for (i, node) in self.nodes.iter().enumerate() {
            node_index.insert(&node.name, i);
        }

        // Build adjacency list and in-degree count
        // Edge: dependency → dependent (if node B references node A, edge A → B)
        let mut in_degree = vec![0usize; n];
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];

        for (i, node) in self.nodes.iter().enumerate() {
            let mut refs = HashSet::new();
            node.expr.collect_refs(&mut refs);
            for r in &refs {
                if let Some(&dep_idx) = node_index.get(r.as_str()) {
                    // dep_idx is a dependency of node i
                    dependents[dep_idx].push(i);
                    in_degree[i] += 1;
                }
                // References to inputs don't create node-to-node edges
            }
        }

        // Kahn's algorithm: start with nodes that have no node dependencies
        let mut queue: VecDeque<usize> = VecDeque::new();
        for i in 0..n {
            if in_degree[i] == 0 {
                queue.push_back(i);
            }
        }

        let mut order = Vec::with_capacity(n);
        while let Some(idx) = queue.pop_front() {
            order.push(idx);
            for &dep in &dependents[idx] {
                in_degree[dep] -= 1;
                if in_degree[dep] == 0 {
                    queue.push_back(dep);
                }
            }
        }

        if order.len() == n {
            Ok(order)
        } else {
            // Cycle detected — collect names of unprocessed nodes
            let processed: HashSet<usize> = order.iter().copied().collect();
            let cycle_nodes: Vec<String> = (0..n)
                .filter(|i| !processed.contains(i))
                .map(|i| self.nodes[i].name.clone())
                .collect();
            Err(cycle_nodes)
        }
    }

    /// Infer types for all nodes in topological order
    fn infer_types(&mut self, errors: &mut Vec<FlowError>) {
        // Build type map from inputs
        let mut type_map: HashMap<String, FlowType> = HashMap::new();
        for input in &self.inputs {
            let ty = input.ty.unwrap_or_else(|| match &input.source {
                FlowInputSource::Builtin(b) => b.output_type(),
                FlowInputSource::Buffer { ty, .. } => *ty,
                FlowInputSource::CssProperty(_) | FlowInputSource::EnvVar(_) => FlowType::Float,
                FlowInputSource::Auto => FlowType::Float,
            });
            type_map.insert(input.name.clone(), ty);
        }

        // Process nodes in topological order
        for &idx in &self.topo_order {
            let node = &self.nodes[idx];
            match infer_expr_type(&node.expr, &type_map) {
                Ok(ty) => {
                    type_map.insert(node.name.clone(), ty);
                    // Store inferred type back
                    self.nodes[idx].inferred_type = Some(ty);
                }
                Err(msg) => {
                    errors.push(FlowError::TypeMismatch {
                        in_node: node.name.clone(),
                        message: msg,
                    });
                    // Default to Float so we can continue checking
                    type_map.insert(node.name.clone(), FlowType::Float);
                    self.nodes[idx].inferred_type = Some(FlowType::Float);
                }
            }
        }
    }

    fn check_function_args(&self, errors: &mut Vec<FlowError>) {
        for node in &self.nodes {
            check_args_recursive(&node.expr, &node.name, errors);
        }
    }

    fn check_outputs(&self, errors: &mut Vec<FlowError>) {
        match self.target {
            FlowTarget::Fragment => {
                // Fragment flows should have a color output
                let has_color = self
                    .outputs
                    .iter()
                    .any(|o| o.target == FlowOutputTarget::Color);
                if !has_color && self.outputs.is_empty() {
                    errors.push(FlowError::MissingOutput {
                        message: "fragment flow needs at least one output (e.g., 'output color')"
                            .to_string(),
                    });
                }
            }
            FlowTarget::Compute => {
                // Compute flows should have buffer outputs
                let has_buffer = self
                    .outputs
                    .iter()
                    .any(|o| matches!(o.target, FlowOutputTarget::Buffer { .. }));
                if !has_buffer && self.outputs.is_empty() {
                    errors.push(FlowError::MissingOutput {
                        message: "compute flow needs at least one buffer output".to_string(),
                    });
                }
            }
        }
    }
}

/// Infer the type of an expression given a type map of known names
fn infer_expr_type(
    expr: &FlowExpr,
    type_map: &HashMap<String, FlowType>,
) -> Result<FlowType, String> {
    match expr {
        FlowExpr::Float(_) => Ok(FlowType::Float),
        FlowExpr::Vec2(_, _) => Ok(FlowType::Vec2),
        FlowExpr::Vec3(_, _, _) => Ok(FlowType::Vec3),
        FlowExpr::Vec4(_, _, _, _) => Ok(FlowType::Vec4),
        FlowExpr::Color(_, _, _, _) => Ok(FlowType::Vec4),

        FlowExpr::Ref(name) => type_map
            .get(name)
            .copied()
            .ok_or_else(|| format!("undefined reference '{}'", name)),

        FlowExpr::Neg(a) => infer_expr_type(a, type_map),

        FlowExpr::Swizzle(_expr, components) => {
            match components.len() {
                1 => Ok(FlowType::Float),
                2 => Ok(FlowType::Vec2),
                3 => Ok(FlowType::Vec3),
                4 => Ok(FlowType::Vec4),
                _ => Err(format!("invalid swizzle length: {}", components.len())),
            }
        }

        FlowExpr::Add(a, b) | FlowExpr::Sub(a, b) | FlowExpr::Mul(a, b) | FlowExpr::Div(a, b) => {
            let ta = infer_expr_type(a, type_map)?;
            let tb = infer_expr_type(b, type_map)?;
            ta.broadcast_with(&tb)
                .ok_or_else(|| format!("cannot broadcast {} with {}", ta, tb))
        }

        FlowExpr::Call { func, args } => {
            let arg_types: Vec<FlowType> = args
                .iter()
                .map(|a| infer_expr_type(a, type_map))
                .collect::<Result<Vec<_>, _>>()?;
            func.return_type(&arg_types)
                .ok_or_else(|| format!("cannot determine return type for {:?}", func))
        }
    }
}

/// Recursively check function argument counts
fn check_args_recursive(expr: &FlowExpr, node_name: &str, errors: &mut Vec<FlowError>) {
    match expr {
        FlowExpr::Call { func, args } => {
            let (min, max) = func.arg_count();
            if args.len() < min || args.len() > max {
                errors.push(FlowError::ArgCountMismatch {
                    in_node: node_name.to_string(),
                    func: format!("{:?}", func),
                    expected: (min, max),
                    got: args.len(),
                });
            }
            for arg in args {
                check_args_recursive(arg, node_name, errors);
            }
        }
        FlowExpr::Add(a, b)
        | FlowExpr::Sub(a, b)
        | FlowExpr::Mul(a, b)
        | FlowExpr::Div(a, b)
        | FlowExpr::Vec2(a, b) => {
            check_args_recursive(a, node_name, errors);
            check_args_recursive(b, node_name, errors);
        }
        FlowExpr::Vec3(a, b, c) => {
            check_args_recursive(a, node_name, errors);
            check_args_recursive(b, node_name, errors);
            check_args_recursive(c, node_name, errors);
        }
        FlowExpr::Vec4(a, b, c, d) => {
            check_args_recursive(a, node_name, errors);
            check_args_recursive(b, node_name, errors);
            check_args_recursive(c, node_name, errors);
            check_args_recursive(d, node_name, errors);
        }
        FlowExpr::Neg(a) | FlowExpr::Swizzle(a, _) => check_args_recursive(a, node_name, errors),
        _ => {}
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ripple_flow() -> FlowGraph {
        let mut graph = FlowGraph::new("ripple");
        graph.target = FlowTarget::Fragment;

        graph.inputs.push(FlowInput {
            name: "uv".to_string(),
            source: FlowInputSource::Builtin(BuiltinVar::Uv),
            ty: Some(FlowType::Vec2),
        });
        graph.inputs.push(FlowInput {
            name: "time".to_string(),
            source: FlowInputSource::Builtin(BuiltinVar::Time),
            ty: Some(FlowType::Float),
        });

        // node dist = distance(uv, vec2(0.5, 0.5))
        graph.nodes.push(FlowNode {
            name: "dist".to_string(),
            expr: FlowExpr::Call {
                func: FlowFunc::Distance,
                args: vec![
                    FlowExpr::Ref("uv".to_string()),
                    FlowExpr::Vec2(
                        Box::new(FlowExpr::Float(0.5)),
                        Box::new(FlowExpr::Float(0.5)),
                    ),
                ],
            },
            inferred_type: None,
        });

        // node wave = sin(dist * 20.0 - time * 4.0)
        graph.nodes.push(FlowNode {
            name: "wave".to_string(),
            expr: FlowExpr::Call {
                func: FlowFunc::Sin,
                args: vec![FlowExpr::Sub(
                    Box::new(FlowExpr::Mul(
                        Box::new(FlowExpr::Ref("dist".to_string())),
                        Box::new(FlowExpr::Float(20.0)),
                    )),
                    Box::new(FlowExpr::Mul(
                        Box::new(FlowExpr::Ref("time".to_string())),
                        Box::new(FlowExpr::Float(4.0)),
                    )),
                )],
            },
            inferred_type: None,
        });

        // node falloff = smoothstep(0.6, 0.0, dist)
        graph.nodes.push(FlowNode {
            name: "falloff".to_string(),
            expr: FlowExpr::Call {
                func: FlowFunc::Smoothstep,
                args: vec![
                    FlowExpr::Float(0.6),
                    FlowExpr::Float(0.0),
                    FlowExpr::Ref("dist".to_string()),
                ],
            },
            inferred_type: None,
        });

        // node height = wave * falloff * 0.15
        graph.nodes.push(FlowNode {
            name: "height".to_string(),
            expr: FlowExpr::Mul(
                Box::new(FlowExpr::Mul(
                    Box::new(FlowExpr::Ref("wave".to_string())),
                    Box::new(FlowExpr::Ref("falloff".to_string())),
                )),
                Box::new(FlowExpr::Float(0.15)),
            ),
            inferred_type: None,
        });

        graph.outputs.push(FlowOutput {
            name: "color".to_string(),
            target: FlowOutputTarget::Color,
            expr: Some(FlowExpr::Vec4(
                Box::new(FlowExpr::Ref("height".to_string())),
                Box::new(FlowExpr::Ref("height".to_string())),
                Box::new(FlowExpr::Ref("height".to_string())),
                Box::new(FlowExpr::Float(1.0)),
            )),
        });

        graph
    }

    // -----------------------------------------------------------------------
    // Valid DAG
    // -----------------------------------------------------------------------

    #[test]
    fn test_valid_dag_validates() {
        let mut graph = make_ripple_flow();
        assert!(graph.validate().is_ok());
    }

    #[test]
    fn test_topo_order_correct() {
        let mut graph = make_ripple_flow();
        graph.validate().unwrap();
        // dist must come before wave, falloff, and height
        let dist_pos = graph.topo_order.iter().position(|&i| i == 0).unwrap();
        let wave_pos = graph.topo_order.iter().position(|&i| i == 1).unwrap();
        let falloff_pos = graph.topo_order.iter().position(|&i| i == 2).unwrap();
        let height_pos = graph.topo_order.iter().position(|&i| i == 3).unwrap();
        assert!(dist_pos < wave_pos);
        assert!(dist_pos < falloff_pos);
        assert!(wave_pos < height_pos);
        assert!(falloff_pos < height_pos);
    }

    #[test]
    fn test_type_inference() {
        let mut graph = make_ripple_flow();
        graph.validate().unwrap();

        assert_eq!(graph.nodes[0].inferred_type, Some(FlowType::Float)); // dist = distance(vec2, vec2)
        assert_eq!(graph.nodes[1].inferred_type, Some(FlowType::Float)); // wave = sin(float)
        assert_eq!(graph.nodes[2].inferred_type, Some(FlowType::Float)); // falloff = smoothstep(...)
        assert_eq!(graph.nodes[3].inferred_type, Some(FlowType::Float)); // height = float * float
    }

    // -----------------------------------------------------------------------
    // Cycle detection
    // -----------------------------------------------------------------------

    #[test]
    fn test_cycle_detected() {
        let mut graph = FlowGraph::new("cyclic");
        graph.target = FlowTarget::Fragment;

        graph.inputs.push(FlowInput {
            name: "x".to_string(),
            source: FlowInputSource::Builtin(BuiltinVar::Time),
            ty: Some(FlowType::Float),
        });

        // a depends on b, b depends on a → cycle
        graph.nodes.push(FlowNode {
            name: "a".to_string(),
            expr: FlowExpr::Add(
                Box::new(FlowExpr::Ref("b".to_string())),
                Box::new(FlowExpr::Float(1.0)),
            ),
            inferred_type: None,
        });
        graph.nodes.push(FlowNode {
            name: "b".to_string(),
            expr: FlowExpr::Add(
                Box::new(FlowExpr::Ref("a".to_string())),
                Box::new(FlowExpr::Float(1.0)),
            ),
            inferred_type: None,
        });

        graph.outputs.push(FlowOutput {
            name: "color".to_string(),
            target: FlowOutputTarget::Color,
            expr: Some(FlowExpr::Ref("a".to_string())),
        });

        let result = graph.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, FlowError::CycleDetected { .. })));
    }

    #[test]
    fn test_self_reference_cycle() {
        let mut graph = FlowGraph::new("self-ref");
        graph.target = FlowTarget::Fragment;

        // node a = a + 1 → self-reference cycle
        graph.nodes.push(FlowNode {
            name: "a".to_string(),
            expr: FlowExpr::Add(
                Box::new(FlowExpr::Ref("a".to_string())),
                Box::new(FlowExpr::Float(1.0)),
            ),
            inferred_type: None,
        });

        graph.outputs.push(FlowOutput {
            name: "color".to_string(),
            target: FlowOutputTarget::Color,
            expr: Some(FlowExpr::Ref("a".to_string())),
        });

        let result = graph.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_three_node_cycle() {
        let mut graph = FlowGraph::new("triangle");
        graph.target = FlowTarget::Fragment;

        // a → b → c → a
        graph.nodes.push(FlowNode {
            name: "a".to_string(),
            expr: FlowExpr::Ref("c".to_string()),
            inferred_type: None,
        });
        graph.nodes.push(FlowNode {
            name: "b".to_string(),
            expr: FlowExpr::Ref("a".to_string()),
            inferred_type: None,
        });
        graph.nodes.push(FlowNode {
            name: "c".to_string(),
            expr: FlowExpr::Ref("b".to_string()),
            inferred_type: None,
        });

        graph.outputs.push(FlowOutput {
            name: "color".to_string(),
            target: FlowOutputTarget::Color,
            expr: Some(FlowExpr::Ref("a".to_string())),
        });

        let result = graph.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        let cycle_err = errors
            .iter()
            .find(|e| matches!(e, FlowError::CycleDetected { .. }));
        assert!(cycle_err.is_some());
        if let FlowError::CycleDetected { nodes } = cycle_err.unwrap() {
            assert_eq!(nodes.len(), 3);
        }
    }

    // -----------------------------------------------------------------------
    // Error cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_undefined_reference() {
        let mut graph = FlowGraph::new("bad-ref");
        graph.target = FlowTarget::Fragment;

        graph.nodes.push(FlowNode {
            name: "a".to_string(),
            expr: FlowExpr::Ref("nonexistent".to_string()),
            inferred_type: None,
        });

        graph.outputs.push(FlowOutput {
            name: "color".to_string(),
            target: FlowOutputTarget::Color,
            expr: Some(FlowExpr::Ref("a".to_string())),
        });

        let result = graph.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, FlowError::UndefinedReference { .. })));
    }

    #[test]
    fn test_duplicate_name() {
        let mut graph = FlowGraph::new("dup");
        graph.target = FlowTarget::Fragment;

        graph.inputs.push(FlowInput {
            name: "x".to_string(),
            source: FlowInputSource::Auto,
            ty: Some(FlowType::Float),
        });
        graph.nodes.push(FlowNode {
            name: "x".to_string(), // conflicts with input
            expr: FlowExpr::Float(1.0),
            inferred_type: None,
        });

        graph.outputs.push(FlowOutput {
            name: "color".to_string(),
            target: FlowOutputTarget::Color,
            expr: Some(FlowExpr::Ref("x".to_string())),
        });

        let result = graph.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, FlowError::DuplicateName { .. })));
    }

    #[test]
    fn test_type_broadcast() {
        // vec3 * float should broadcast to vec3
        let mut graph = FlowGraph::new("broadcast");
        graph.target = FlowTarget::Fragment;

        graph.inputs.push(FlowInput {
            name: "uv".to_string(),
            source: FlowInputSource::Builtin(BuiltinVar::Uv),
            ty: Some(FlowType::Vec2),
        });

        // node scaled = uv * 2.0 → should be Vec2
        graph.nodes.push(FlowNode {
            name: "scaled".to_string(),
            expr: FlowExpr::Mul(
                Box::new(FlowExpr::Ref("uv".to_string())),
                Box::new(FlowExpr::Float(2.0)),
            ),
            inferred_type: None,
        });

        graph.outputs.push(FlowOutput {
            name: "color".to_string(),
            target: FlowOutputTarget::Color,
            expr: Some(FlowExpr::Ref("scaled".to_string())),
        });

        graph.validate().unwrap();
        assert_eq!(graph.nodes[0].inferred_type, Some(FlowType::Vec2));
    }

    #[test]
    fn test_incompatible_types() {
        let mut graph = FlowGraph::new("bad-types");
        graph.target = FlowTarget::Fragment;

        graph.inputs.push(FlowInput {
            name: "a".to_string(),
            source: FlowInputSource::Auto,
            ty: Some(FlowType::Vec2),
        });
        graph.inputs.push(FlowInput {
            name: "b".to_string(),
            source: FlowInputSource::Auto,
            ty: Some(FlowType::Vec3),
        });

        // vec2 + vec3 → type error
        graph.nodes.push(FlowNode {
            name: "c".to_string(),
            expr: FlowExpr::Add(
                Box::new(FlowExpr::Ref("a".to_string())),
                Box::new(FlowExpr::Ref("b".to_string())),
            ),
            inferred_type: None,
        });

        graph.outputs.push(FlowOutput {
            name: "color".to_string(),
            target: FlowOutputTarget::Color,
            expr: Some(FlowExpr::Ref("c".to_string())),
        });

        let result = graph.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, FlowError::TypeMismatch { .. })));
    }

    // -----------------------------------------------------------------------
    // Compute target
    // -----------------------------------------------------------------------

    #[test]
    fn test_compute_flow_needs_buffer_output() {
        let mut graph = FlowGraph::new("compute-no-output");
        graph.target = FlowTarget::Compute;
        graph.workgroup_size = Some(64);

        graph.inputs.push(FlowInput {
            name: "pos".to_string(),
            source: FlowInputSource::Buffer {
                name: "positions".to_string(),
                ty: FlowType::Vec4,
            },
            ty: Some(FlowType::Vec4),
        });

        graph.nodes.push(FlowNode {
            name: "new_pos".to_string(),
            expr: FlowExpr::Add(
                Box::new(FlowExpr::Ref("pos".to_string())),
                Box::new(FlowExpr::Float(1.0)),
            ),
            inferred_type: None,
        });

        // No output → should fail
        let result = graph.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_flow_with_buffer_output() {
        let mut graph = FlowGraph::new("compute-ok");
        graph.target = FlowTarget::Compute;
        graph.workgroup_size = Some(64);

        graph.inputs.push(FlowInput {
            name: "pos".to_string(),
            source: FlowInputSource::Buffer {
                name: "positions".to_string(),
                ty: FlowType::Vec4,
            },
            ty: Some(FlowType::Vec4),
        });

        graph.nodes.push(FlowNode {
            name: "new_pos".to_string(),
            expr: FlowExpr::Add(
                Box::new(FlowExpr::Ref("pos".to_string())),
                Box::new(FlowExpr::Float(1.0)),
            ),
            inferred_type: None,
        });

        graph.outputs.push(FlowOutput {
            name: "positions".to_string(),
            target: FlowOutputTarget::Buffer {
                name: "positions".to_string(),
            },
            expr: Some(FlowExpr::Ref("new_pos".to_string())),
        });

        assert!(graph.validate().is_ok());
    }

    // -----------------------------------------------------------------------
    // Empty graph
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_fragment_flow_errors() {
        let mut graph = FlowGraph::new("empty");
        graph.target = FlowTarget::Fragment;

        let result = graph.validate();
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // FlowFunc parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_flowfunc_from_str() {
        assert_eq!(FlowFunc::from_str("sin"), Some(FlowFunc::Sin));
        assert_eq!(FlowFunc::from_str("distance"), Some(FlowFunc::Distance));
        assert_eq!(
            FlowFunc::from_str("sdf-smooth-union"),
            Some(FlowFunc::SdfSmoothUnion)
        );
        assert_eq!(
            FlowFunc::from_str("sdf_smooth_union"),
            Some(FlowFunc::SdfSmoothUnion)
        );
        assert_eq!(FlowFunc::from_str("unknown"), None);
    }

    // -----------------------------------------------------------------------
    // FlowType broadcast
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_broadcast_rules() {
        assert_eq!(
            FlowType::Float.broadcast_with(&FlowType::Float),
            Some(FlowType::Float)
        );
        assert_eq!(
            FlowType::Float.broadcast_with(&FlowType::Vec3),
            Some(FlowType::Vec3)
        );
        assert_eq!(
            FlowType::Vec3.broadcast_with(&FlowType::Float),
            Some(FlowType::Vec3)
        );
        assert_eq!(FlowType::Vec2.broadcast_with(&FlowType::Vec3), None);
        assert_eq!(
            FlowType::Vec4.broadcast_with(&FlowType::Vec4),
            Some(FlowType::Vec4)
        );
    }
}

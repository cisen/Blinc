//! Canvas toolkit for interactive pan/zoom canvases.
//!
//! `blinc_canvas_kit` provides viewport management (pan, zoom, coordinate
//! conversion) for `blinc_layout::Canvas` elements. All state persists across
//! UI rebuilds via `BlincContextState`.
//!
//! # Usage
//!
//! ## Builder (zero boilerplate)
//!
//! ```ignore
//! use blinc_canvas_kit::prelude::*;
//!
//! fn my_canvas() -> Div {
//!     let kit = CanvasKit::new("diagram");
//!     kit.element(|ctx, bounds| {
//!         // Draw in content-space (viewport transform pre-applied)
//!         ctx.fill_rect(Rect::new(100.0, 100.0, 200.0, 150.0), 8.0.into(), Brush::Solid(Color::BLUE));
//!     })
//! }
//! ```
//!
//! ## Handler (custom wiring)
//!
//! ```ignore
//! use blinc_canvas_kit::prelude::*;
//!
//! fn my_canvas() -> Div {
//!     let kit = CanvasKit::new("custom");
//!     let h = kit.handler();
//!     div()
//!         .on_drag(h.clone())
//!         .on_scroll(h.clone())
//!         .child(canvas(move |ctx, bounds| {
//!             ctx.push_transform(kit.transform().into());
//!             // draw ...
//!             ctx.pop_transform();
//!         }))
//! }
//! ```

pub mod pan;
pub mod viewport;
pub mod zoom;

use std::rc::Rc;
use std::sync::Arc;

use blinc_core::events::event_types;
use blinc_core::layer::{Affine2D, Point};
use blinc_core::{BlincContextState, SignalId, State};
use blinc_layout::canvas::{canvas, CanvasBounds};
use blinc_layout::div::{div, Div};
use blinc_layout::event_handler::EventContext;

pub use pan::PanController;
pub use viewport::{affine_inverse, CanvasViewport};
pub use zoom::ZoomController;

/// Prelude for convenient imports.
pub mod prelude {
    pub use crate::pan::PanController;
    pub use crate::viewport::{affine_inverse, CanvasViewport};
    pub use crate::zoom::ZoomController;
    pub use crate::CanvasKit;
}

/// Interactive canvas toolkit — receives events, manages viewport state.
///
/// Create one per canvas via `CanvasKit::new("key")`. All state persists
/// across UI rebuilds via `BlincContextState`.
///
/// Two ways to use:
/// - `kit.element(render_fn)` — returns a Div with Canvas + event handlers pre-wired
/// - `kit.handler()` — returns a closure to attach to any element
#[derive(Clone)]
pub struct CanvasKit {
    viewport: State<CanvasViewport>,
    pan: State<PanController>,
    zoom_controller: ZoomController,
}

impl CanvasKit {
    /// Create a canvas kit with persistent state keyed by name.
    pub fn new(key: &str) -> Self {
        let ctx = BlincContextState::get();
        Self {
            viewport: ctx.use_state_keyed(&format!("{key}_vp"), CanvasViewport::new),
            pan: ctx.use_state_keyed(&format!("{key}_pan"), PanController::new),
            zoom_controller: ZoomController::new(),
        }
    }

    /// Create with custom zoom controller settings.
    pub fn with_zoom_controller(mut self, zc: ZoomController) -> Self {
        self.zoom_controller = zc;
        self
    }

    // ── Event Processing ──────────────────────────────────────────

    /// Process an EventContext directly. Dispatches to pan/zoom based on event_type.
    pub fn handle_event(&self, evt: &EventContext) {
        match evt.event_type {
            event_types::DRAG => {
                let dx = evt.drag_delta_x;
                let dy = evt.drag_delta_y;
                // Update pan controller (velocity tracking) and viewport together
                self.pan.update(|mut pan| {
                    let mut vp = self.viewport.get();
                    pan.on_drag(&mut vp, dx, dy);
                    self.viewport.set(vp);
                    pan
                });
            }
            event_types::DRAG_END => {
                self.pan.update(|mut pan| {
                    pan.on_drag_end();
                    pan
                });
            }
            event_types::SCROLL => {
                let delta_y = evt.scroll_delta_y;
                let cursor = Point::new(evt.local_x, evt.local_y);
                let zc = self.zoom_controller.clone();
                self.viewport.update(|mut vp| {
                    zc.on_scroll(&mut vp, delta_y, cursor);
                    vp
                });
            }
            event_types::PINCH => {
                let scale = evt.pinch_scale;
                let center = Point::new(evt.pinch_center_x, evt.pinch_center_y);
                let zc = self.zoom_controller.clone();
                self.viewport.update(|mut vp| {
                    zc.on_pinch(&mut vp, scale, center);
                    vp
                });
            }
            _ => {}
        }
    }

    /// Returns a handler closure suitable for attaching to any element.
    ///
    /// The returned closure handles DRAG, DRAG_END, SCROLL, and PINCH events.
    ///
    /// ```ignore
    /// let kit = CanvasKit::new("my_canvas");
    /// let h = kit.handler();
    /// div().on_drag(h.clone()).on_scroll(h.clone()).on_drag_end(h.clone())
    /// ```
    pub fn handler(&self) -> Arc<dyn Fn(&EventContext) + Send + Sync + 'static> {
        let kit = self.clone();
        Arc::new(move |evt: &EventContext| {
            kit.handle_event(evt);
        })
    }

    // ── Builder ───────────────────────────────────────────────────

    /// Build a fully-wired Div containing a Canvas with all event handlers.
    ///
    /// The render callback receives `DrawContext` with the viewport transform
    /// already applied — draw in content-space coordinates directly.
    ///
    /// The returned Div is `w_full().h_full()`. Chain additional builder
    /// methods to customize sizing.
    pub fn element<F>(&self, render_fn: F) -> Div
    where
        F: Fn(&mut dyn blinc_core::DrawContext, CanvasBounds) + 'static,
    {
        let kit_render = self.clone();
        let render = Rc::new(render_fn);

        let h_drag = self.handler();
        let h_drag_end = self.handler();
        let h_scroll = self.handler();
        let h_pinch = self.handler();

        div()
            .w_full()
            .h_full()
            .on_drag(move |evt| h_drag(evt))
            .on_drag_end(move |evt| h_drag_end(evt))
            .on_scroll(move |evt| h_scroll(evt))
            .on_event(event_types::PINCH, move |evt| h_pinch(evt))
            .child(
                canvas(move |ctx, bounds| {
                    let transform = kit_render.transform();
                    ctx.push_transform(transform.into());
                    render(ctx, bounds);
                    ctx.pop_transform();
                })
                .w_full()
                .h_full(),
            )
    }

    // ── Viewport Access ───────────────────────────────────────────

    /// Current viewport transform (content → screen).
    pub fn transform(&self) -> Affine2D {
        self.viewport.get().transform()
    }

    /// Current viewport state.
    pub fn viewport(&self) -> CanvasViewport {
        self.viewport.get()
    }

    /// Update the viewport via a mutation closure.
    pub fn update_viewport(&self, f: impl FnOnce(&mut CanvasViewport)) {
        self.viewport.update(|mut vp| {
            f(&mut vp);
            vp
        });
    }

    /// Signal ID for reactive dependency tracking (e.g. `stateful().deps()`).
    pub fn viewport_signal(&self) -> SignalId {
        self.viewport.signal_id()
    }

    /// Convert screen-space point to content-space.
    pub fn screen_to_content(&self, screen: Point) -> Point {
        self.viewport.get().screen_to_content(screen)
    }

    /// Convert content-space point to screen-space.
    pub fn content_to_screen(&self, content: Point) -> Point {
        self.viewport.get().content_to_screen(content)
    }
}

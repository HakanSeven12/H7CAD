use super::render::{CameraState, PaperViewportPrimitive, Primitive};
use super::Scene;
use acadrust::Handle;
use iced::widget::shader;
use iced::{mouse, Event, Rectangle};

// ── Mode ──────────────────────────────────────────────────────────────────

pub enum ViewportPaneMode {
    /// Full model space — fills whatever bounds Iced assigns.
    Model,
    /// Model-space content rendered through a specific viewport's 3-D camera.
    ///
    /// NOTE: Currently unused because Iced 0.14 batches all shader `prepare()`
    /// calls before any `render()` calls. Multiple widgets sharing the same
    /// `Pipeline` type overwrite each other's GPU buffers.  A per-viewport
    /// wgpu sub-renderer that accumulates data across frames would be needed to
    /// revive this path.
    #[allow(dead_code)]
    Paper { handle: Handle },
}

// ── Widget struct ─────────────────────────────────────────────────────────

pub struct ViewportPane<'a> {
    pub scene: &'a Scene,
    pub mode: ViewportPaneMode,
}

impl<'a> ViewportPane<'a> {
    pub fn model(scene: &'a Scene) -> Self {
        Self { scene, mode: ViewportPaneMode::Model }
    }

    /// One paper-space viewport: model content rendered through its own camera.
    /// See [`ViewportPaneMode::Paper`] for why this is currently unused.
    #[allow(dead_code)]
    pub fn paper(scene: &'a Scene, handle: Handle) -> Self {
        Self { scene, mode: ViewportPaneMode::Paper { handle } }
    }
}

// ── PaperViewportPane ─────────────────────────────────────────────────────
//
// A shader widget for the MSPACE active viewport.  Uses PaperViewportPrimitive
// (and therefore PaperViewportPipeline) so it gets its own Iced storage entry,
// separate from the ViewportPane/PaperSheet pipeline.

pub struct PaperViewportPane<'a> {
    pub scene: &'a Scene,
    pub handle: Handle,
}

impl<'a> PaperViewportPane<'a> {
    pub fn new(scene: &'a Scene, handle: Handle) -> Self {
        Self { scene, handle }
    }
}

impl<'a, Msg: std::fmt::Debug + Clone> shader::Program<Msg> for PaperViewportPane<'a> {
    type State = CameraState;
    type Primitive = PaperViewportPrimitive;

    fn draw(
        &self,
        state: &Self::State,
        _cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        self.scene.build_active_viewport_primitive(self.handle, state.hover_region, bounds)
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<Msg>> {
        self.scene.update_viewcube_state(state, bounds, cursor);
        let _ = event;
        None
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        _b: Rectangle,
        _c: mouse::Cursor,
    ) -> mouse::Interaction {
        self.scene.viewcube_mouse_interaction(state)
    }
}

// ── ViewportPane shader::Program impl ────────────────────────────────────

impl<'a, Msg: std::fmt::Debug + Clone> shader::Program<Msg> for ViewportPane<'a> {
    type State = CameraState;
    type Primitive = Primitive;

    fn draw(
        &self,
        state: &Self::State,
        _cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        match &self.mode {
            ViewportPaneMode::Model => {
                self.scene.build_primitive(state.hover_region, bounds)
            }
            ViewportPaneMode::Paper { handle } => {
                self.scene.build_viewport_primitive(*handle, state.hover_region, bounds)
            }
        }
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<Msg>> {
        // ViewCube hover only makes sense in the full model-space view.
        if matches!(self.mode, ViewportPaneMode::Model) {
            self.scene.update_viewcube_state(state, bounds, cursor);
        }
        let _ = event;
        None
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        _b: Rectangle,
        _c: mouse::Cursor,
    ) -> mouse::Interaction {
        if matches!(self.mode, ViewportPaneMode::Model) {
            self.scene.viewcube_mouse_interaction(state)
        } else {
            mouse::Interaction::default()
        }
    }
}

//! Removed: multi-backend selector and OpenGL references.
//! WGPU is the only supported backend.

#[allow(dead_code)]
pub enum RenderBackend {
    Wgpu,
}

#[allow(dead_code)]
pub struct BackendSelector;

impl BackendSelector {
    pub fn new(_prefer_wgpu: bool, _renderer_preference: Option<()>) -> Self {
        Self
    }
    pub fn select_backend(&self) -> RenderBackend {
        RenderBackend::Wgpu
    }
}

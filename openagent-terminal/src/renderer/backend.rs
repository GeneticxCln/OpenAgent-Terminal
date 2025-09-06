use std::fmt;
use tracing::{debug, info};

use crate::config::debug::RendererPreference;

/// Available rendering backends
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    /// WebGPU-based renderer (modern, cross-platform)
    Wgpu,
    /// OpenGL-based renderer (legacy fallback)
    OpenGl,
}

impl fmt::Display for RenderBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderBackend::Wgpu => write!(f, "WGPU"),
            RenderBackend::OpenGl => write!(f, "OpenGL"),
        }
    }
}

/// Backend selection strategy
#[allow(dead_code)]
pub struct BackendSelector {
    prefer_wgpu: bool,
    renderer_preference: Option<RendererPreference>,
}

impl BackendSelector {
    pub fn new(prefer_wgpu: bool, renderer_preference: Option<RendererPreference>) -> Self {
        Self { prefer_wgpu, renderer_preference }
    }

    /// Select the appropriate backend based on availability and preferences
    pub fn select_backend(&self) -> RenderBackend {
        // Check if user has explicit OpenGL preference
        if let Some(pref) = self.renderer_preference {
            match pref {
                RendererPreference::Glsl3
                | RendererPreference::Gles2
                | RendererPreference::Gles2Pure => {
                    debug!("User preference forces OpenGL backend");
                    return RenderBackend::OpenGl;
                },
            }
        }

        // If WGPU is preferred and available, use it
        #[cfg(feature = "wgpu")]
        {
            if self.prefer_wgpu && Self::is_wgpu_available() {
                info!("Selected WGPU backend");
                return RenderBackend::Wgpu;
            }
        }

        // Fallback to OpenGL
        info!("Selected OpenGL backend (fallback)");
        RenderBackend::OpenGl
    }

    /// Check if WGPU is available on the current system
    #[allow(dead_code)]
    #[cfg(feature = "wgpu")]
    fn is_wgpu_available() -> bool {
        // Try to create a WGPU instance to check availability
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter_result =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            }));

        if adapter_result.is_ok() {
            debug!("WGPU backend is available");
            true
        } else {
            tracing::warn!("WGPU backend not available on this system");
            false
        }
    }

    #[allow(dead_code)]
    #[cfg(not(feature = "wgpu"))]
    fn is_wgpu_available() -> bool {
        false
    }
}

/// Backend-specific configuration
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct BackendConfig {
    pub backend: RenderBackend,
    pub vsync: bool,
    pub max_texture_size: Option<u32>,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self { backend: RenderBackend::OpenGl, vsync: true, max_texture_size: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_prefers_gl_when_not_preferred() {
        let sel = BackendSelector::new(false, None);
        assert_eq!(sel.select_backend(), RenderBackend::OpenGl);
    }

    #[cfg(not(feature = "wgpu"))]
    #[test]
    fn selector_falls_back_to_gl_without_wgpu_feature() {
        let sel = BackendSelector::new(true, None);
        // Without the feature, selector must choose OpenGL even if preferred.
        assert_eq!(sel.select_backend(), RenderBackend::OpenGl);
    }

    #[cfg(feature = "wgpu")]
    #[test]
    fn selector_handles_wgpu_preference_smoke() {
        // Should not panic; returns either Wgpu (if available) or OpenGl.
        let sel = BackendSelector::new(true, None);
        let backend = sel.select_backend();
        match backend {
            RenderBackend::Wgpu | RenderBackend::OpenGl => {},
        }
    }
}

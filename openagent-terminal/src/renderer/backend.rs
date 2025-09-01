use std::fmt;
use tracing::{info, warn, debug};

use crate::config::debug::RendererPreference;

/// Available rendering backends
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
pub struct BackendSelector {
    prefer_wgpu: bool,
    renderer_preference: Option<RendererPreference>,
}

impl BackendSelector {
    pub fn new(prefer_wgpu: bool, renderer_preference: Option<RendererPreference>) -> Self {
        Self {
            prefer_wgpu,
            renderer_preference,
        }
    }

    /// Select the appropriate backend based on availability and preferences
    pub fn select_backend(&self) -> RenderBackend {
        // Check if user has explicit OpenGL preference
        if let Some(pref) = self.renderer_preference {
            match pref {
                RendererPreference::Glsl3 | 
                RendererPreference::Gles2 | 
                RendererPreference::Gles2Pure => {
                    debug!("User preference forces OpenGL backend");
                    return RenderBackend::OpenGl;
                }
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
    #[cfg(feature = "wgpu")]
    fn is_wgpu_available() -> bool {
        // Try to create a WGPU instance to check availability
        match wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        }).request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }) {
            Some(_) => {
                debug!("WGPU backend is available");
                true
            }
            None => {
                warn!("WGPU backend not available on this system");
                false
            }
        }
    }

    #[cfg(not(feature = "wgpu"))]
    fn is_wgpu_available() -> bool {
        false
    }
}

/// Backend-specific configuration
#[derive(Debug, Clone)]
pub struct BackendConfig {
    pub backend: RenderBackend,
    pub vsync: bool,
    pub max_texture_size: Option<u32>,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            backend: RenderBackend::OpenGl,
            vsync: true,
            max_texture_size: None,
        }
    }
}

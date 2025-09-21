#![allow(clippy::pedantic)]

// Minimal WGPU initialization example (requires --features=wgpu)

#[cfg(feature = "wgpu")]
fn main() {
    // Initialize WGPU instance, adapter, and device without a window; exit on success.
    // This validates that the WGPU stack can be created in the current environment.
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    // Request a high-performance adapter (headless)
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("request_adapter failed");

    let _device_and_queue = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("OpenAgent WGPU Example Device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: Default::default(),
        trace: Default::default(),
    }))
    .expect("Failed to create WGPU device for example");
}

#[cfg(not(feature = "wgpu"))]
fn main() {
    // Example requires the 'wgpu' feature
}

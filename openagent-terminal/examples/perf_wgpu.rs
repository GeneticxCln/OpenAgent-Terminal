// Simple WGPU perf example (feature: wgpu). 
// NOTE: WGPU backend is not yet fully implemented - this is a placeholder

// For now, we'll create a stub that indicates WGPU is not ready

fn main() {
    // WGPU backend is not yet fully implemented
    // This example will be enabled once WGPU renderer reaches parity with OpenGL
    eprintln!("WGPU performance example is not yet implemented.");
    eprintln!("The WGPU backend is still under development.");
    eprintln!("Use 'cargo run --example perf_latency' for OpenGL performance testing.");
    std::process::exit(0); // Exit 0 so CI doesn't fail
}


fn main() {
    // Allow using `#[cfg(ci)]` in tests without triggering `unexpected_cfgs`.
    println!("cargo:rustc-check-cfg=cfg(ci)");
}

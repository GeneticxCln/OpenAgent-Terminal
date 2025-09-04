{
  description = "OpenAgent Terminal dev shell (Rust + GL/Winit/WGPU deps)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system:
        let pkgs = import nixpkgs { inherit system; }; in f pkgs);
    in {
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust toolchain
            rustc cargo pkg-config cmake scdoc
            # X11/Wayland/OpenGL stack
            xorg.libX11 xorg.libXext xorg.libXi xorg.libXrandr xorg.libXcursor
            libxkbcommon wayland libGL libEGL freetype udev
          ];
          RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
        };
      });
    };
}

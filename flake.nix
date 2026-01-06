{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
        in
        with pkgs;
        {
          formatter = nixpkgs.legacyPackages.${system}.nixpkgs-fmt;
          devShells.default = mkShell {
            buildInputs = [
              rust-bin.stable.latest.default
              fontconfig
              freetype
              libglvnd
              libinput
              libxkbcommon
              vulkan-loader
              wayland
              xorg.libX11
            ];

            nativeBuildInputs = [
              pkg-config
            ];

            RUSTFLAGS = map (a: "-C link-arg=${a}") [
              "-Wl,--push-state,--no-as-needed"
              "-lEGL"
              "-lwayland-client"
              "-Wl,--pop-state"
            ];

            LD_LIBRARY_PATH = lib.makeLibraryPath [
              libxkbcommon
              mesa
              vulkan-loader
              xorg.libX11
              xorg.libXcursor
              xorg.libXi
              xorg.libXrandr
            ];
          };
        }
      );
}

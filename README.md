# cosmic-term
WIP COSMIC terminal emulator, built using [alacritty\_terminal](https://docs.rs/alacritty_terminal) that is provided by the [alacritty](https://github.com/alacritty/alacritty) project. `cosmic-term` provides bidirectional rendering and ligatures with a custom renderer based on [cosmic-text](https://github.com/pop-os/cosmic-text).

The `wgpu` feature, enabled by default, supports GPU rendering using `glyphon`
and `wgpu`. If `wgpu` is not enabled or fails to initialize, then rendering falls
back to using `softbuffer` and `tiny-skia`.

## Color Schemes

Custom color schemes can be imported from the `View -> Color schemes...` menu item.
You can find templates for color schemes in the [color-schemes](color-schemes) folder.

## Build on Ubuntu 24.04 LTS (Wayland) Minimal Dependencies

After extensive testing, the minimal set of dependencies required to build `cosmic-term` on Ubuntu 24.04 (Wayland) is as follows:

1.  **Install Minimal Dependencies:**
    ```bash
    sudo apt install libxkbcommon-dev libwayland-dev pkg-config libgl-dev libegl1-mesa-dev
    ```

2.  **Set PKG_CONFIG_PATH:**
    The build requires the `PKG_CONFIG_PATH` to be explicitly set to resolve the `xkbcommon.pc` dependency. This step must be run in the terminal session before executing `cargo run`.
    ```bash
    export PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig
    ```

3.  **Build and Run:**
    ```bash
    cargo run --release
    ```

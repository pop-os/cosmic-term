# cosmic-term (macOS Port)

This is a fork of `cosmic-term` focused on porting the emulator to run natively on **macOS**.

Built using the robust [alacritty_terminal](https://docs.rs/alacritty_terminal) backend (from [alacritty](https://github.com/alacritty/alacritty)), this project adapts the original Linux-focused codebase to function as a standalone application on Mac. It retains the advanced bidirectional rendering and ligatures powered by [cosmic-text](https://github.com/pop-os/cosmic-text).

**Goal:** Remove dependencies on the COSMIC Desktop Environment (Linux/Wayland specific) and ensure smooth integration with macOS windowing and input.

## Rendering

The `wgpu` feature is enabled by default and optimized to use **Metal** on macOS for high-performance GPU rendering via `glyphon`.

If `wgpu` is not enabled or fails to initialize, rendering falls back to using `softbuffer` and `tiny-skia`.

## Color Schemes

Custom color schemes can be imported from the `View -> Color schemes...` menu item.
You can find templates for color schemes in the [color-schemes](color-schemes) folder.
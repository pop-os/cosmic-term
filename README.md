# cosmic-term
WIP COSMIC terminal emulator, built using [alacritty\_terminal](https://docs.rs/alacritty_terminal) that is provided by the [alacritty](https://github.com/alacritty/alacritty) project. `cosmic-term` provides bidirectional rendering and ligatures with a custom renderer based on [cosmic-text](https://github.com/pop-os/cosmic-text).

The `wgpu` feature, enabled by default, supports GPU rendering using `glyphon`
and `wgpu`. If `wgpu` is not enabled or fails to initialize, then rendering falls
back to using `softbuffer` and `tiny-skia`.

## Color Schemes

Custom color schemes can be imported from the `View -> Color schemes...` menu item.
You can find templates for color schemes in the [color-schemes](color-schemes) folder.

## Keyboard Shortcuts

Custom key bindings can be configured in `~/.config/cosmic-term/cosmic-term.yaml`.  
Currently, only single-character keys are supported—keys like `"ArrowLeft"`, `"Enter"`, or `"Backspace"` are not yet available.

#### Example
```yaml
key_bindings:
  - key: V   # Pressing Ctrl+Shift+Super+Alt+V pastes
    mods: Ctrl|Shift|Super|Alt
    action: Paste
    
  - key: F   # Pressing Alt+F zooms in
    mods: Alt
    action: ZoomIn
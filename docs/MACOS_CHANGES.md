# macOS Porting Changes

This document details the modifications made to the `cosmic-term` codebase to enable support for macOS (Darwin).

## 1. Dependencies (`Cargo.toml`)

-   **`objc` crate**: Added `objc = "0.2"` under `[target.'cfg(target_os = "macos")'.dependencies]` to enable Objective-C runtime interaction for window activation.
-   **`cosmic-files`**: Vendorized locally to resolve dependency conflicts or apply patches if needed (currently using `path = "vendor/cosmic-files"`).
-   **`fontconfig` / `freetype`**: Configured features to avoid linking issues where possible, relying on system font loading.

## 2. Application Initialization (`src/main.rs`)

### Shell Detection Fallback
Modified `main()` to detect viable shells on macOS, as standard Linux paths might not exist or user configuration files (like `.zshrc`) might cause issues in the raw PTY environment.
-   Checks for `fish` in Homebrew paths (`/opt/homebrew/bin/fish`).
-   Fallbacks to `/bin/bash` if standard shell detection fails.

### Window Activation Policy (Keyboard Fix)
**Critical Fix:** macOS applications launched directly from a terminal binary (via `cargo run` or raw executable) do not automatically acquire the "Regular" activation policy, meaning they don't appear in the Dock and **do not receive keyboard focus**.

Implemented a fix in `App::init` using the `objc` crate:
-   Accesses `NSApplication.sharedApplication`.
-   Sets `setActivationPolicy:` to `NSApplicationActivationPolicyRegular` (0).
-   Calls `activateIgnoringOtherApps:YES` to steal focus immediately upon launch.

This ensures the terminal window receives key events (like typing) instead of them being captured by the parent terminal running `cargo run`.

### Daemonization
-   Disabled daemonization logic (`fork`) on macOS, as strictly required by `launchd` and macOS application lifecycle guidelines.

## 3. Terminal & PTY
-   The underlying `alacritty_terminal` crate handles PTY interactions.
-   Linking against `libxkbcommon` is required manually via `RUSTFLAGS` due to path differences on macOS (Homebrew location).

## 4. Debugging Artifacts
-   Temporary `println!` debug logs were injected into `src/main.rs`, `src/terminal.rs`, and `src/terminal_box.rs` to diagnose the event loop flow but have been removed in the final source.

## 5. macOS Native Window Decorations (Traffic Lights) & Border Radius
-   **Client Server Decorations**: Disabled via `settings = settings.client_decorations(false);` in `src/main.rs`. This shifts responsibility to macOS for drawing the native title bar.
-   **`CALayer` Bottom Corners**: Updated the `setMaskedCorners` Objective-C call in `Message::WindowFocused` to use the correct bitmask `1 | 2` (`kCALayerMinXMaxYCorner | kCALayerMaxXMaxYCorner`). This correctly restores bottom rounded corners without affecting the sharp top corners needed for the native title bar.
-   **LibCosmic `sharp_corners` Race Condition**: To avoid visual gaps where libcosmic draws rounded top corners beneath the macOS native title bar:
    - Added a `#[cfg(target_os = "macos")]` block to force `self.core.window.sharp_corners = true` at the very end of the main `update()` loop.
    - This neutralizes libcosmic's internal `WindowMaximized(id, false)` event that continuously attempted to disable sharp corners on non-maximized windows.
-   **Terminal Inner Padding Radius**: Modified `radius` in `src/terminal_box.rs` inside the `draw` routine to ensure `[0.0, 0.0, corner_radius[2], corner_radius[3]]` is used on macOS.
-   **Container Limitations**: Removed an invalid `.border()` setting on the `widget::tab_bar::horizontal` container to fix an `[E0599]` compilation error.

## 6. Native File Menu / Global Menu Bar (AppKit NSMenu)
- To replace the libcosmic integrated widget menu bar to behave like a native macOS Application, the following changes were applied:
    - **`Cargo.toml`**: Added target-specific crate `muda` (Menu Utilities for Desktop Applications) to handle Cocoa/Objective-C `NSMenu` construction.
    - **`src/main.rs`:**
        - Modified `App` struct to store `Option<muda::Menu>` on macOS so the native menu outlives the initialization stack.
        - Modified `App::init` to set `settings.show_headerbar = false` by default on macOS, neatly turning off the internal LibCosmic menu logic while still enabling user toggles, rather than stripping `header_start()` out entirely.
        - `update(&mut self)`: Interrogates `muda::MenuEvent::receiver().try_recv()` natively during the application loop, mapping incoming action IDs (e.g., "TabNew", "WindowNew") to regular cosmic-term `Action::xyz.message()` variants.
    - **`src/mac_menu.rs`**: Created a macOS-exclusive module to initialize the `muda::Menu`. Added `.ok().flatten()` logic instead of `unwrap()` to prevent `UnsupportedKey` panics during shortcut mappings.

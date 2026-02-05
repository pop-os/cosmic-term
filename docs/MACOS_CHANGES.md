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

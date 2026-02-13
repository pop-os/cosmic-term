# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
# Build (release)
just build-release          # or: cargo build --release

# Build (debug)
just build-debug            # or: cargo build

# Run with debug logging
just run                    # Sets RUST_LOG=cosmic_term=debug, RUST_BACKTRACE=full

# Lint (pedantic clippy)
just check                  # or: cargo clippy --all-features -- -W clippy::pedantic

# Format code
cargo fmt

# Clean build artifacts
just clean                  # or: cargo clean
```

## Architecture Overview

### Core Components

**`src/main.rs`** - Application entry point and main UI logic
- Contains `App` struct (main application state)
- `Action` enum - All keyboard/menu actions (wired to `Message` in `Action::message()`)
- `Message` enum - All application events
- `update()` method - Central message handler
- `view()` method - UI rendering with `PaneGrid`

**`src/terminal.rs`** - Terminal emulation and pane management
- `TerminalPaneGrid` - Manages split panes, each containing a `TabModel`
- `Terminal` - Individual terminal instance with `tab_title_override: Option<String>`
- `TabModel` - Type alias for `segmented_button::Model<segmented_button::SingleSelect>`

**`src/config.rs`** - Configuration and profiles
- `Config` - Main config with `profiles: BTreeMap<ProfileId, Profile>`
- `Profile` - Contains `tab_title: String` for default tab titles
- `ColorScheme` - Dark/Light color schemes

**`src/key_bind.rs`** - Keyboard shortcuts
- `key_binds()` returns `HashMap<KeyBind, Action>`
- Use `bind!([Modifiers...], Key::..., Action)` macro

**`src/menu.rs`** - Context menu and menu bar
- `context_menu()` - Right-click menu on terminal
- `menu_bar()` - Top application menu bar

### Data Flow Patterns

**Tab Title Priority:**
1. `Terminal.tab_title_override` (highest - user-set)
2. `Profile.tab_title` (profile default)
3. Terminal OSC title sequence (dynamic from shell)

**Action → Message Flow:**
1. Keyboard shortcut or menu item triggers `Action`
2. `Action::message(entity_opt)` converts to `Message`
3. `App::update(Message)` handles the event

**Split/Tab Hierarchy:**
```
PaneGrid
 └── Pane (split container)
     └── TabModel (tabs within pane)
         └── Terminal (per tab, stored as data<Mutex<Terminal>>)
```

### Message Handler Pattern

When adding new features with user input (e.g., rename dialogs):

1. **Add Action variant** in `Action` enum (`src/main.rs:~220`)
2. **Add Message variants** - typically 3: `Start`, `Submit`, `Cancel` (`src/main.rs:~330`)
3. **Add state fields** to `App` struct: `{name}_id: widget::Id`, `{name}_rename: Option<(...)>` (`src/main.rs:~470`)
4. **Initialize in `App::new()`** (`src/main.rs:~1590`)
5. **Wire Action→Message** in `Action::message()` (`src/main.rs:~268`)
6. **Add key binding** in `src/key_bind.rs` using `bind!` macro
7. **Add menu item** in `src/menu.rs:context_menu()` or `menu_bar()`
8. **Implement handlers** in `update()` method (`src/main.rs:~1635`)
9. **Add Escape handler** in `on_escape()` if cancellable (`src/main.rs:~1615`)
10. **Add UI** in `view()` method (`src/main.rs:~2811`)
11. **Add i18n keys** to `i18n/*/cosmic_term.ftl`

### Key Types

- `segmented_button::Entity` - Tab identifier in `TabModel`
- `pane_grid::Pane` - Split pane identifier
- `ProfileId` - Wrapper around `u64`
- `ColorSchemeId` - Wrapper around `u64`

### Localization

All user-facing strings use `fl!("key")` macro. Add keys to:
- `i18n/en/cosmic_term.ftl` (English source)
- `i18n/*/cosmic_term.ftl` (other locales)

### GPU Rendering

Default: `wgpu` feature via `glyphon`
Fallback: `softbuffer` + `tiny-skia` (if wgpu fails)

Based on `alacritty_terminal` for VTTE emulation, `cosmic-text` for rendering.

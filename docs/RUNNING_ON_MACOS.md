# Running Cosmic Term on macOS

This document outlines the steps required to build and run `cosmic-term` on macOS.

## Prerequisites

1.  **Rust & Cargo**: Ensure you have a recent version of Rust installed via `rustup`.
2.  **Homebrew**: Required for installing system dependencies.
3.  **libxkbcommon**: This library is required for keyboard handling.

```bash
brew install libxkbcommon
```

## Running Development Build

Due to linking requirements with `libxkbcommon`, you must specify the library path in `RUSTFLAGS` when running the application.

```fish
# In Fish shell
set -x RUSTFLAGS "-L "(brew --prefix libxkbcommon)"/lib"
cargo run
```

```bash
# In Bash/Zsh
export RUSTFLAGS="-L $(brew --prefix libxkbcommon)/lib"
cargo run
```

## Building for Release

To create an optimized release binary:

```fish
set -x RUSTFLAGS "-L "(brew --prefix libxkbcommon)"/lib"
cargo build --release
```

The binary will be located at `target/release/cosmic-term`.

## Known Issues

-   **Window Icon**: The application uses a generic executable icon when run specifically from the target directory. Creating a `.app` bundle would resolve this.
-   **Fonts**: Ensure a suitable monospace font is installed. The application defaults to system monospace priorities.
-   **Daemonization**: Daemon mode is disabled on macOS to comply with platform lifecycle management.

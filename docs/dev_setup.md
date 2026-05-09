# Local Development Setup

This repo is a Rust and Bevy learning harness for the Copper Island economy prototype. It keeps the deterministic simulation in plain Rust and uses Bevy only for visualization, input, and debug UI.

## Current Machine Notes

The first scaffold was prepared on CachyOS, an Arch-like x86_64 Linux distribution. VS Code, `clang`, and `lld` were already available, but `rustup`, `rustc`, and `cargo` were not on `PATH`.

## Install Rust

Use `rustup` so the repo can pin a stable toolchain with `rust-toolchain.toml`.

```bash
sudo pacman -S rustup
rustup default stable
rustup component add rustfmt clippy rust-src
```

Open a new shell after installing `rustup`, then confirm:

```bash
rustup show active-toolchain
rustc --version
cargo --version
```

## Install Bevy Linux Dependencies

For Arch/CachyOS:

```bash
sudo pacman -S libx11 pkgconf alsa-lib libxcursor libxrandr libxi clang lld
```

Install one ALSA bridge depending on your audio stack:

```bash
sudo pacman -S pipewire-alsa
```

or:

```bash
sudo pacman -S pulseaudio-alsa
```

Install the Vulkan driver package for your GPU. Examples:

```bash
sudo pacman -S vulkan-radeon
sudo pacman -S vulkan-intel
```

If Bevy starts but cannot find a GPU, confirm the correct Vulkan driver is installed for the machine.

## First Manual Gates

These commands are intentionally manual so the first Bevy compile can be watched.

```bash
cargo check --workspace
cargo test --workspace
cargo run -p bevy_client
```

The first `cargo check` after adding Bevy can take a while because it builds engine dependencies. Later checks should be much faster, especially with the checked-in `lld` linker config.

## VS Code

Open the repo folder in VS Code. The checked-in `.vscode/extensions.json` should prompt for recommended extensions.

Recommended baseline:

- rust-analyzer for Rust language support.
- CodeLLDB for debugging.
- Even Better TOML for `Cargo.toml` and config files.
- Markdown tooling for the learning notes.

No `.code-profile` is committed. If you want one later, create it locally from VS Code's Profiles UI and export it only if the settings remain non-sensitive and broadly useful.

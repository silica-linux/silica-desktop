<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/silica-logo-dark.svg">
    <img alt="Silica" src=".github/silica-logo-light.svg" width="200">
  </picture>
</p>

<h1 align="center">Silica Desktop</h1>

<p align="center">
  <b>A Linux desktop environment written from scratch in Rust and Flutter.</b><br>
  A glass visual language, measured on real observations and reimplemented clean-room.
</p>

<p align="center">
  <img alt="Rust: compositor" src="https://img.shields.io/badge/Rust-compositor-b7410e?style=flat-square&logo=rust">
  <img alt="Flutter: shell & apps" src="https://img.shields.io/badge/Flutter-shell%20%26%20apps-02569B?style=flat-square&logo=flutter">
  <img alt="Linux: Wayland" src="https://img.shields.io/badge/Linux-Wayland-2f7bf5?style=flat-square&logo=linux">
  <img alt="license: MIT / Apache-2.0" src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-3a3a3a?style=flat-square">
  <img alt="status: early alpha" src="https://img.shields.io/badge/status-early%20alpha-d69e2e?style=flat-square">
</p>

| Part | What it does |
| --- | --- |
| `crates/ossido` | the Wayland compositor (Smithay): windows, decorations, spaces, Panorama, drag and drop between apps |
| `crates/ossido-render` | the GLES renderer: glass, shadows, window title bars |
| `crates/embedder` | the Flutter embedder for the shell and the apps |
| `crates/daemon` | system services over D-Bus: network, audio, brightness, appearance |
| `crates/portal` | the xdg-desktop-portal backend |
| `crates/control`, `crates/cli` | the control channel, `silica-cli` and the MCP server |
| `shell/` | the Flutter shell: menu bar, Dock, Faro, notifications, Control Center |

## Try it

Nested inside another desktop:

```sh
cargo xtask run --nested --shell
```

The Logo key is the right Ctrl when nested. Working rules are in [`AGENTS.md`](AGENTS.md),
documentation in [`docs/`](docs/); the other repositories (silica-ui, silica-icons, Mustache,
the distribution) are listed in `AGENTS.md`.

## License

MIT OR Apache-2.0. Every asset has a row in [`ASSETS.md`](ASSETS.md).

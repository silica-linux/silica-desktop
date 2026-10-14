# AGENTS.md

The working contract of every session, for people and agents. Silica is a Linux desktop
environment written from scratch in Rust, with the visual language of macOS 27
(Liquid Glass). The compositor is called Ossido.

## Session start

1. Look at the project board (https://github.com/orgs/silica-linux/projects/1): milestones and
   issues of every repository are the plan. `plan/STATUS.md` and `plan/execution-plan.md`
   (local, gitignored) keep only the context.
2. For the expected behaviour: `plan/reference/*.md` (measured observations of macOS 27).
3. Work autonomously; stop only for the decisions the plan assigns to the user.

## Principles

1. Clean-room: macOS is observed from the outside, not copied.
2. Never crash: a compositor crash closes the whole session.
3. Everything is a command: every user action is a command reachable from the UI,
   the CLI, the control channel and MCP.
4. Parity is measured, not declared: screenshots compared with the references and
   conformance tests, never percentages written by agents.

## Never crash

- In non-test code no `unwrap`, `expect`, `panic!`, `todo!`, `unreachable!`;
  `unsafe` is forbidden except for the embedder's FFI (`crates/embedder/src/ffi/`) and
  Ossido's EGL/GLES initialization (`crates/ossido/src/egl.rs`), isolated in a module
  with a `SAFETY` comment for every block. Clippy enforces it (see `[workspace.lints]` in `Cargo.toml`).
- Errors as `Result` with `?`; input from Wayland clients, files and commands treated as
  hostile (checked indices, checked arithmetic).
- A misbehaving client is disconnected, never the compositor going down.
- Every crash fixed comes with a regression test.

## Everything is a command

- Every visible action is a command with an id, label, shortcut, parameters and tests.
  UI, CLI, control channel and MCP call the same commands.

## Quality gates

- Before every commit: `cargo xtask ci`. One task per commit (`M2.3: glass refraction`).
- For visual work: screenshot, reading the PNG, comparison with the reference before
  closing the task.
- Parity in `ROADMAP.md` is updated only from measured metrics, never estimated by the agent.
- Every bug, gap or provisional value has an issue on GitHub (shared labels, milestone, on the
  board); commits that close one say `Fixes silica-linux/<repo>#N`. Measurement requests are
  issues in silica-reference (label `measurement`). Never paste reference captures or text
  into public repositories.
- English everywhere: code, comments, tests, docs, commits. User-visible texts are localized:
  English source, Italian translation.

## Agents in parallel

- One `CARGO_TARGET_DIR` per agent; each agent changes only the crates or packages
  assigned to it; the build directory is deleted at the end of the work.

## Clean-room and assets

Layouts and behaviours may be reimplemented, assets may not.

Allowed observation:

- macOS 27 is observed from the outside on a real Apple Silicon Mac: use of the UI,
  screenshots, measurements (sizes, radii, spacings, durations and curves of the animations).
- The observations go in `plan/reference/` (gitignored): screenshots and measured notes,
  never committed.
  Procedure, conditions and format: `docs/reference-capture.md`.
- Public specifications: Wayland protocols, freedesktop specifications, Smithay and
  Flutter documentation.
- Apple's public documentation (developer.apple.com, Human Interface Guidelines,
  WWDC sessions): only to know which behaviours exist and how they are used (e.g. glass
  shapes that merge, glass reacting to touch). No texts, images, code or names are copied
  from it; the values (sizes, curves, colors) are still measured from observation.

Forbidden:

- Reading, disassembling or copying files from the macOS system (bundles, frameworks, resources).
- Apple icons, wallpapers, sounds, cursors, texts and fonts. The SF fonts have a license
  limited to Apple platforms: Inter (OFL) takes their place for the interface.
- Apple product names in the interface and in marketing.
- Code with an incompatible license: no copying from GPL projects (COSMIC, KWin,
  GNOME Shell) into an MIT/Apache repo. Reading their architecture is fine, copying their
  code is not.

Assets:

- Every asset has a row in `ASSETS.md` with author, license and source;
  `cargo xtask assets` (part of `cargo xtask ci`) blocks files without a row.
- Allowed licenses: the project's own original work, CC0, CC-BY, CC BY-SA, OFL, MIT/Apache.
  Graphics generated in code are preferred.
- App, file and system icons: repo `silica-linux/silica-icons`, license
  CC BY-SA 4.0. Cursors are postponed: the repo `silica-linux/silica-cursors` is archived.
- Release wallpapers: our own photos or photographers' with a written transfer of rights,
  or Wikimedia Commons with a compatible license.

## Structure and layers

```
crates/ossido/         compositor (Smithay)
crates/ossido-render/  GLES renderer + glass shaders
crates/protocols/      XML and bindings of the silica-* protocols
crates/embedder/       Flutter embedder in Rust
crates/daemon/         system services (D-Bus)
crates/portal/         xdg-desktop-portal backend
crates/control/        JSON control channel + MCP server
crates/cli/            silica-cli
shell/                 the shell in Flutter: Dock, menu bar, Faro, notifications, Control Center
testapps/demo/         test app for the embedder tests
xtask/                 cargo xtask: ci, layers, assets, run, bundle, vm, os
plan/                  gitignored: status, plan, macOS references
```

`cargo xtask layers` checks the layers: no crate below the shell depends on
Flutter, and `ossido` does not depend on `daemon`.

This repository is `silica-desktop` (until 2026-10-14 `silica`). The other repositories of
`silica-linux`:

| Repository | What | License |
| --- | --- | --- |
| `silica-ui` (public) | Dart package `silica_ui`: Inter, glass, lens glass. Stable API, versions tagged `vX.Y.Z`; the shell and the apps take it from the tag | MIT OR Apache-2.0 |
| `silica-icons` | app and file icons (layered), 313 system symbols in three weights | CC BY-SA 4.0 |
| `silica-cursors` | cursors, archived: to be redone later | CC BY-SA 4.0 |
| `silica-reference` | macOS 27 observations, cloned into `plan/reference/` (private) | — |
| `silica-packages` | the RPM packages (desktop, apps, Inter, silica-icons) and the Wayland session installable on Fedora | MIT OR Apache-2.0 |
| `silica-os` | the distro: a bootc image on top of Fedora Atomic with Silica's defaults, pipeline and installation ISO | MIT OR Apache-2.0 |
| `silica-brand` | logo, repository banners, release wallpapers (with their rows in `ASSETS.md`) | Silica trademark, all rights reserved (`TRADEMARKS.md`) |
| `silica-setup` | the first-boot assistant (an app, like `mustache`) | MIT OR Apache-2.0 |
| `settings`, `mail`, `terminal`, `clock`, `calculator` | the Settings, Mail (Flutter app + Rust engine), Terminal, Clock and Calculator apps (like `mustache`) | MIT OR Apache-2.0 |
| `.github` (public) | default issue templates for every repository | — |
| one repository per app | `mustache` (public, the file manager), then Settings, Terminal, …: they depend on `silica-ui`; they are built with `cargo xtask bundle <path>`; `cargo xtask install-app <path>` installs their `.desktop` entry (template in the app repository's `linux/`) | MIT OR Apache-2.0 |

Faro and the other shell components are not apps: they stay in `shell/`. A change to
`silica-ui` is released with a tag before using it here (`ref` in `shell/pubspec.yaml`).

## Develop and test in Silica OS

Every change is built here and checked inside the real Silica OS before it goes to CI: one
VM, started with `cargo xtask os start` and kept open on the user's screen (they watch the
work), updated with `cargo xtask os deploy [--apps]`, inspected with `cargo xtask os shot`,
`cmd`, `ssh` and `log` (docs/vm.md, "Development loop"). Push only after the change works
there; a release is booted and looked at in the VM (icons, Dock, menus, apps) before it is
tagged. The nested and headless sessions below remain for quick checks and CI.

## Harness

1. `cargo xtask run --nested --control 7979` starts Ossido nested (winit backend) with
   the control channel.
2. The agent sends JSON commands on the socket (`engine.execute`, `ui.screenshot`), with
   `silica-cli` or via MCP (`silica-cli mcp`, already in `.mcp.json`). Protocol and commands:
   `docs/control.md`.
3. The screenshot is compared with the reference in `plan/reference/` with a
   visual difference metric; the result updates the parity in `ROADMAP.md`.
4. Headless, the same loop runs in CI, without a window.

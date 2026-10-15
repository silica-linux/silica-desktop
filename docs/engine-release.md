# Flutter release engine

Google publishes `libflutter_engine.so` for custom embedders (`linux-x64-embedder.zip`)
only in debug mode: Dart in JIT, with assertions, 43 MB. To measure and distribute we need
the release engine, which is built from source at the revision in
`crates/embedder/ENGINE_VERSION`. Release apps run with their AOT-compiled Dart
(`libapp.so`).

Measured on 2026-10-12 on WSL2 (20 threads, 15 GB): sync 9.4 GB, build about 15 minutes,
release library 17.8 MB.

## Preparing the checkout, once per version

The engine lives in the `flutter/flutter` monorepo, under `engine/src`. The SDK checkout
already has the revision: a worktree is enough.

```bash
git clone --depth 1 https://chromium.googlesource.com/chromium/tools/depot_tools.git ~/build/depot_tools
git -C ~/sdk/flutter worktree add --detach ~/build/flutter-engine "$(cat crates/embedder/ENGINE_VERSION)"
cat > ~/build/flutter-engine/.gclient <<'EOF'
solutions = [{
  "name": ".", "url": "https://github.com/flutter/flutter.git",
  "deps_file": "DEPS", "managed": False, "custom_deps": {}, "safesync_url": "",
  "custom_vars": {"download_android_deps": False, "download_jdk": False,
                  "download_fuchsia_deps": False, "download_esbuild": False,
                  "setup_githooks": False},
}]
EOF
cd ~/build/flutter-engine && PATH=~/build/depot_tools:$PATH DEPOT_TOOLS_UPDATE=0 \
  gclient sync -D --no-history --shallow -j 8
```

## Building and using it

```bash
SILICA_ENGINE_SRC=~/build/flutter-engine cargo xtask engine --release
cargo xtask bundle dock --release        # apps/dock/build/release/{flutter_assets,libapp.so}
silica-embedder --engine target/flutter-engine/<ENGINE_VERSION>-release \
  --renderer impeller apps/dock/build/release/flutter_assets
```

The embedder finds `libapp.so` next to `flutter_assets` (or with `--aot`). With the debug
engine the AOT code is ignored; with the release one the debug bundle is refused.

## Engine patches

`cargo xtask engine --release` applies every `crates/embedder/engine-patches/*.patch` to the
checkout before building (a patch already applied is left alone) and writes their names, one
per line, into `silica-patches` next to `libflutter_engine.so`. The embedder reads that file
to know what the engine can do; the debug engine Google publishes has none of them.

| Patch | What | Upstream |
| --- | --- | --- |
| `impeller-vulkan-backing-stores` | Impeller draws into Vulkan backing stores: apps with several windows and a shell with several layers render with Impeller on Vulkan, with no copy (#203). Without it they draw with Skia on Vulkan | flutter/flutter#193003 (open), issue flutter/flutter#187525 |

When `ENGINE_VERSION` moves to a revision that contains a patch's change, the patch applies
in reverse and is reported as already applied. Delete it then, together with the embedder's
check for it (`engine_has_patch` in `crates/embedder/src/window.rs`): every engine of that
revision, the debug one included, has the change. A patch that neither applies nor is there
stops the build.

## When the Flutter version changes

After the steps in `docs/gate1-embedder.md` (update `ENGINE_VERSION`, `engine`,
`bindings`, `keymap`): move the worktree to the new revision
(`git -C ~/build/flutter-engine checkout --detach <rev>`), `gclient sync` again, then
`cargo xtask engine --release`. It does not run in CI: it takes hours and tens of GB. When
distribution is needed, the release engine will have to be built once per version and
published as a project artifact.

# Engine Flutter release

Google pubblica `libflutter_engine.so` per embedder custom (`linux-x64-embedder.zip`)
solo in modalità debug: Dart in JIT, con asserzioni, 43 MB. Per misurare e distribuire
serve l'engine release, che si compila dai sorgenti alla revisione di
`crates/embedder/ENGINE_VERSION`. Le app release girano con il loro Dart compilato AOT
(`libapp.so`).

Misurato il 2026-10-12 su WSL2 (20 thread, 15 GB): sincronizzazione 9,4 GB, compilazione
circa 15 minuti, libreria release 17,8 MB.

## Preparare il checkout, una volta per versione

L'engine vive nel monorepo `flutter/flutter`, sotto `engine/src`. Il checkout dell'SDK ha
già la revisione: basta un worktree.

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

## Compilare e usare

```bash
SILICA_ENGINE_SRC=~/build/flutter-engine cargo xtask engine --release
cargo xtask bundle dock --release        # apps/dock/build/release/{flutter_assets,libapp.so}
silica-embedder --engine target/flutter-engine/<ENGINE_VERSION>-release \
  --renderer gl apps/dock/build/release/flutter_assets
```

L'embedder trova `libapp.so` accanto a `flutter_assets` (o con `--aot`). Con l'engine
debug il codice AOT si ignora; con quello release il bundle debug viene rifiutato.

## Al cambio di versione di Flutter

Dopo i passi di `docs/gate1-embedder.md` (aggiornare `ENGINE_VERSION`, `engine`,
`bindings`, `keymap`): spostare il worktree alla nuova revisione
(`git -C ~/build/flutter-engine checkout --detach <rev>`), `gclient sync` di nuovo, poi
`cargo xtask engine --release`. In CI non gira: richiede ore e decine di GB. Quando
servirà distribuire, l'engine release andrà compilato una volta per versione e
pubblicato come artefatto del progetto.

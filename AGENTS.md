# AGENTS.md

Contratto di lavoro di ogni sessione, per persone e agenti. Silica è un desktop
environment Linux scritto da zero in Rust, con il linguaggio visivo di macOS 27
(Liquid Glass). Il compositor si chiama Ossido.

## Inizio sessione

1. Leggere `plan/STATUS.md` (milestone corrente, prossimo task), poi il task in
   `plan/execution-plan.md` e la sezione rilevante di `plan/architecture.md`.
2. Per il comportamento atteso: `plan/reference/*.md` (osservazioni misurate di macOS 27).
3. Lavorare in autonomia; fermarsi solo per le decisioni che il piano assegna all'utente.

## Principi

1. Clean-room: macOS si osserva dall'esterno, non si copia.
2. Non crashare mai: un crash del compositor chiude l'intera sessione.
3. Tutto è un comando: ogni azione dell'utente è un comando raggiungibile da UI,
   CLI, canale di controllo e MCP.
4. La parità si misura, non si dichiara: screenshot confrontati con i riferimenti e
   test di conformità, mai percentuali scritte dagli agenti.

## Mai crashare

- Nel codice non di test niente `unwrap`, `expect`, `panic!`, `todo!`, `unreachable!`;
  `unsafe` vietato salvo FFI dell'embedder, isolato in un modulo con commento `SAFETY`
  per ogni blocco. Clippy lo fa rispettare (vedi `[workspace.lints]` in `Cargo.toml`).
- Errori come `Result` con `?`; input da client Wayland, file e comandi trattati come
  ostili (indici controllati, aritmetica checked).
- Un client che si comporta male viene disconnesso, mai il compositor che cade.
- Ogni crash corretto arriva con un test di regressione.

## Tutto è un comando

- Ogni azione visibile è un comando con id, etichetta, scorciatoia, parametri e test.
  UI, CLI, canale di controllo e MCP chiamano gli stessi comandi.

## Gate di qualità

- Prima di ogni commit: `cargo xtask ci`. Un task per commit (`M2.3: rifrazione del vetro`).
- Per il lavoro visivo: screenshot, lettura del PNG, confronto con il riferimento prima
  di chiudere il task.
- La parità in `ROADMAP.md` si aggiorna solo da metriche misurate, mai stimata dall'agente.

## Agenti in parallelo

- Un `CARGO_TARGET_DIR` per agente; ogni agente modifica solo i crate o i pacchetti
  che gli sono assegnati; la directory di build si cancella a fine lavoro.

## Clean-room e asset

Layout e comportamenti si possono reimplementare, gli asset no.

Osservazione consentita:

- macOS 27 si osserva dall'esterno su un Mac Apple Silicon reale: uso della UI,
  screenshot, misure (dimensioni, raggi, spaziature, durate e curve delle animazioni).
- Le osservazioni vanno in `plan/reference/` (gitignored): screenshot e note misurate,
  mai committati.
  Procedura, condizioni e formato: `docs/reference-capture.md`.
- Specifiche pubbliche: protocolli Wayland, specifiche freedesktop, documentazione
  Smithay e Flutter.

Vietato:

- Leggere, disassemblare o copiare file dal sistema macOS (bundle, framework, risorse).
- Icone, sfondi, suoni, cursori, testi e font Apple. I font SF hanno una licenza
  limitata alle piattaforme Apple: al loro posto Inter (OFL) per l'interfaccia.
- Nomi di prodotto Apple nell'interfaccia e nel marketing.
- Codice con licenza incompatibile: niente copia da progetti GPL (COSMIC, KWin,
  GNOME Shell) in un repo MIT/Apache. Leggerne l'architettura è lecito, copiarne il
  codice no.

Asset:

- Ogni asset ha una riga in `ASSETS.md` con autore, licenza e fonte;
  `cargo xtask assets` (parte di `cargo xtask ci`) blocca i file senza riga.
- Licenze ammesse: originale del progetto, CC0, CC-BY, OFL, MIT/Apache. Da preferire
  la grafica generata in codice.
- Sfondi delle versioni: foto proprie o di fotografi con cessione scritta dei diritti,
  oppure Wikimedia Commons con licenza compatibile.

## Struttura e livelli

```
crates/ossido/         compositor (Smithay)
crates/ossido-render/  renderer GLES + shader del vetro
crates/protocols/      XML e binding dei protocolli silica-*
crates/embedder/       embedder Flutter in Rust
crates/daemon/         servizi di sistema (D-Bus)
crates/portal/         backend xdg-desktop-portal
crates/control/        canale di controllo JSON + server MCP
crates/cli/            silica-cli
xtask/                 cargo xtask: ci, layers, assets, run
plan/                  gitignored: stato, piano, riferimenti macOS
```

`cargo xtask layers` verifica i livelli: nessun crate sotto la shell dipende da
Flutter, e `ossido` non dipende da `daemon`.

## Harness

1. `cargo xtask run --nested --control 7979` avvia Ossido annidato (backend winit) con
   canale di controllo.
2. L'agente invia comandi JSON sul socket (`engine.execute`, `ui.screenshot`), con
   `silica-cli` o via MCP (`silica-cli mcp`, già in `.mcp.json`). Protocollo e comandi:
   `docs/control.md`.
3. Lo screenshot viene confrontato con il riferimento in `plan/reference/` con una
   metrica di differenza visiva; il risultato aggiorna la parità in `ROADMAP.md`.
4. In headless lo stesso ciclo gira in CI, senza finestra.

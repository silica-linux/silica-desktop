# Gate 1 · Dock in Flutter e in Rust, misurati

Rapporto di M1.5 (`docs/m1.5-dock.md`): lo stesso Dock scritto in Flutter (`apps/dock`) e
in Rust con iced (`crates/dock-iced`), misurato da Ossido sullo stesso banco di prova
(`cargo xtask bench dock`). Dati del 2026-10-12.

## Raccomandazione

**Opzione B: shell in Rust (iced), app in Flutter.** Sulle misure di oggi iced è più
fluido (nessun frame perso con la GPU, contro l'1,4% di Flutter) e occupa 45 MiB in meno.
La shell gira sempre, ed è la parte dove un rallentamento si vede di più. Il prezzo è più
codice (circa 2,3 volte), un toolkit meno maturo e due design system da tenere allineati.

Due cautele, prima di decidere:

- **Il ritardo di Flutter con la GPU viene dal nostro embedder, non dal disegno.** Nei
  frame persi il thread raster finisce in meno di 12 ms; Flutter in software non perde
  frame. Il tempo si perde fra vsync, swap e compositor, in un percorso che in WSL passa
  da una copia GPU → CPU (Ossido non offre ancora dmabuf). È probabilmente correggibile, e
  va corretto comunque per le app Flutter, in qualunque opzione.
- **Tutto è misurato in WSL**, con OpenGL tradotto in D3D12. I numeri valgono per il
  confronto, non come valori assoluti. La conferma su hardware Linux reale arriva con M6.

Se conta di più la velocità di sviluppo della UI, l'opzione A resta praticabile: il
divario di fluidità è nel nostro codice, quello di memoria (circa 45 MiB, una volta sola
se la shell usa un solo engine) no.

## Banco di prova

Ossido annidato in WSLg, output 2560×1600 a scala 2, refresh misurato circa 56 Hz
(periodo nominale 16,7 ms). Per ogni variante 3 ripetizioni, ciascuna con un Ossido nuovo:

1. lancio del Dock con `app.launch`; avvio = dal lancio al primo buffer;
2. 10 s di puntatore avanti e indietro sulla riga delle icone, 120 eventi/s;
3. uscita del puntatore e ritorno a riposo con la molla (1,5 s);
4. memoria del processo dopo lo scorrimento (`/proc/<pid>/smaps_rollup`).

Le misure sono di Ossido (`perf.frames`, `perf.process`), uguali per ogni toolkit. Un frame
è lento se arriva oltre 1,5 periodi dopo il precedente. Tutto in build release: Rust
`--release`, Flutter con engine release compilato dai sorgenti e Dart AOT.

| Variante | Cosa |
| --- | --- |
| flutter-gl | Flutter, embedder custom, OpenGL ES (Skia), GPU via D3D12 |
| flutter-gl-impeller | come sopra, con Impeller |
| iced-gl | iced 0.14, wgpu su OpenGL ES, GPU via D3D12 |
| flutter-software | Flutter, renderer software (CPU) |
| iced-software | iced 0.14, tiny-skia (CPU) |

## Risultati

Mediana di 3 ripetizioni. Tempi in ms, memoria in MiB.

| Variante | Avvio | Frame | Media | p50 | p95 | p99 | Max | Lenti | PSS | RSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| flutter-gl | 361 | 561 | 17,77 | 17,61 | 20,81 | 30,57 | 33,2 | 8 | 106,5 | 176,1 |
| flutter-gl-impeller | 421 | 561 | 17,78 | 17,66 | 21,63 | 30,95 | 34,1 | 22 | 128,9 | 198,6 |
| iced-gl | 297 | 561 | 17,83 | 17,77 | 19,60 | 21,39 | 23,3 | 0 | 60,8 | 118,9 |
| flutter-software | 20 | 560 | 17,84 | 17,81 | 19,61 | 21,86 | 24,6 | 0 | 33,3 | 39,1 |
| iced-software | 10 | 563 | 17,79 | 17,78 | 19,37 | 21,05 | 22,3 | 0 | 11,4 | 15,0 |

Frame lenti nelle singole ripetizioni: flutter-gl 4, 8, 10; flutter-gl-impeller 36, 12,
22; iced-gl 0, 2, 0; flutter-software 0, 0, 3; iced-software 1, 0, 0.

Con le soglie proposte in `docs/m1.5-dock.md`:

| Soglia | flutter-gl | iced-gl |
| --- | --- | --- |
| Frame lenti < 1% | no (1,4%) | sì (0%) |
| p99 ≤ 1,5 periodi (25 ms) | no (30,6 ms) | sì (21,4 ms) |
| Nessun intervallo oltre 50 ms | sì (33,2 ms) | sì (23,3 ms) |
| Memoria | da fissare: 106,5 MiB | da fissare: 60,8 MiB |

## Cosa si legge nei numeri

- **Fluidità.** Le due versioni stanno al ritmo del compositor (media circa 17,8 ms).
  Flutter con la GPU perde lo 0,7–1,8% dei frame (Impeller il 2–6%), sparsi lungo tutto
  lo scorrimento e non solo all'inizio. In una ripetizione con il log del raster
  (`silica_embedder::ffi::gl` in debug) quei frame hanno un disegno sotto i 12 ms. Impeller, il renderer nuovo, va peggio di Skia in
  questo percorso.
- **Memoria.** Flutter costa circa 45 MiB in più con la GPU e 22 MiB in più in software, per
  processo. PSS include le librerie del driver grafico, condivise fra processi.
- **Avvio.** Con la GPU domina l'inizializzazione del driver D3D12 (200–400 ms, molto
  variabile) per entrambi. In software Flutter AOT parte in 20 ms, iced in 10.
- **Aspetto.** Gli screenshot (`target/bench/dock/*-hover.png`) hanno la stessa geometria.
  In WSL la superficie di iced è opaca: wgpu 27 su OpenGL dichiara solo `Opaque`
  (`wgpu-hal/src/gles/adapter.rs`, un TODO), tiny-skia idem. La trasparenza c'è con Vulkan,
  che in WSL è solo lavapipe (CPU); su hardware reale con Vulkan va verificata.

## Costo di sviluppo

| | Flutter | iced |
| --- | --- | --- |
| Righe (senza test) | 188 | 429 |
| Problemi d'integrazione | 1: font fuori dal pacchetto (risolto con `packages/silica_ui`) | 4: `wp_viewporter` obbligatorio in Ossido, winit-core da fissare a una beta, superficie opaca su GL, `CursorEntered` senza movimento |
| Molla, colori HSL | dal framework | scritti a mano (`geometry.rs`) |
| Tempo dell'agente | circa 5 minuti | circa 15 minuti, più le indagini |

I tempi sono quelli dell'agente che ha scritto i prototipi: indicativi, non una misura.

## Costi scoperti durante M1.5

- **Engine release.** Google pubblica l'engine per embedder custom solo in debug. Il
  release si compila dai sorgenti (`docs/engine-release.md`): 9,4 GB di checkout, circa
  15 minuti, a ogni aggiornamento di Flutter. Serve per le app Flutter in ogni opzione.
- **Percorso OpenGL in WSL.** Mesa (d3d12) applica il ridimensionamento della finestra
  EGL solo dal buffer successivo: l'embedder ora chiede un frame in più dopo ogni resize.
- **Ritmo dei frame.** L'embedder segue i frame callback del compositor
  (`vsync_callback`), come fa iced; prima l'engine andava con un suo timer a 60 Hz.

## Cosa resta aperto

- Il percorso GPU dell'embedder: dmabuf in Ossido e `wp_presentation` per il ritmo dei
  frame, poi una nuova misura. Serve in ogni opzione, per le app.
- La misura su hardware Linux reale (M6).
- Il limite di memoria per la shell, che fissi tu.

## Ripetere le misure

```bash
SILICA_ENGINE_SRC=~/build/flutter-engine cargo xtask engine --release
cargo xtask bundle dock --release
cargo xtask bench dock                 # --runs N --seconds S --only VARIANTE
```

Risultati in `target/bench/dock/results.json`, screenshot accanto.

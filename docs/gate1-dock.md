# Gate 1 · Dock in Flutter e in Rust, misurati

Rapporto di M1.5 (`docs/m1.5-dock.md`): lo stesso Dock scritto in Flutter (`apps/dock`) e
in Rust con iced (`crates/dock-iced`), misurato da Ossido sullo stesso banco di prova
(`cargo xtask bench dock`). Dati del 2026-10-12.

## Decisione (2026-10-13)

**Opzione A: shell e app in Flutter**, con l'embedder custom. Decisa dall'utente dopo le
misure su hardware nativo (sezione più sotto) e una prova a mano dei due Dock.

Perché, rispetto alla raccomandazione iniziale qui sotto:

- Il vantaggio di fluidità di iced non si ripete su Linux nativo: con lo stesso renderer
  (llvmpipe) le due versioni perdono gli stessi frame.
- A vista iced rende peggio: le forme del canvas non hanno antialiasing (MSAA spento di
  default, accenderlo ha un costo da rimisurare) e il testo di cosmic-text/cryoglyph,
  senza hinting né correzione gamma, è meno nitido di quello di Skia con lo stesso Inter.
- iced chiede più codice (2,3 volte), cose scritte a mano che Flutter dà già, e un
  secondo design system da tenere allineato a quello delle app.
- I costi di Flutter (engine release, percorso GPU dell'embedder, dmabuf in Ossido) si
  pagano comunque per le app.

Condizioni:

- `linux-dmabuf` in Ossido, poi una nuova misura del Dock Flutter con la GPU vera su
  hardware nativo: frame lenti sotto l'1%.
- **Impeller, non Skia** (decisione dell'utente). Impeller è il renderer su cui Flutter
  investe; Skia è il ripiego che verrà tolto. Quindi il crash di Impeller su llvmpipe e i
  frame persi in WSL (2–6%, peggio di Skia) vanno capiti e risolti, non aggirati. La
  misura con la GPU nativa si fa con flutter-gl-impeller, e va valutato Impeller su
  Vulkan (l'API embedder ha un renderer Vulkan) oltre che su OpenGL ES.
- Il limite di memoria della shell resta da fissare (oggi circa 110 MiB di PSS per il
  Dock con la GPU in WSL, da rimisurare con la GPU nativa).

## Raccomandazione iniziale (2026-10-12, superata)

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
  confronto, non come valori assoluti. Su Linux nativo (sezione più sotto) la GPU non è
  ancora raggiungibile dai client di Ossido.

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

## Misure su hardware nativo (2026-10-12)

Stesso banco, stesso PC, con Ubuntu 26.04 nativo al posto di WSL: Core i9-13900H
(20 thread), Iris Xe (Raptor Lake-P) e RTX 4070 Laptop, Mesa 26.0.8, driver NVIDIA 595.91. Ossido annidato nella sessione GNOME (Wayland), schermo 1920×1200 a 59,88 Hz.

**Le varianti "GPU" qui non usano la GPU.** Ossido non offre `zwp_linux_dmabuf_v1` (né
`wl_drm`), e senza uno dei due Mesa non sa quale device usare per una superficie Wayland:
l'EGL dei client ricade su llvmpipe (`libEGL warning: failed to get driver name for fd -1`,
e l'embedder scrive `renderer OpenGL ES: llvmpipe`). Con Vulkan wgpu sceglie lavapipe; con
solo il driver Intel o solo quello NVIDIA nessuna superficie Vulkan è utilizzabile e iced
ripiega su tiny-skia (PSS 14–16 MiB). In WSL il problema non si vedeva perché il driver
d3d12 di Mesa non ha bisogno di dmabuf e copia sempre il frame in memoria condivisa.

**Scala 1 invece di 2.** GNOME stringe la finestra 2560×1600 di Ossido a 1920×1082, e il
puntatore del banco cadeva fuori dal Dock (0 frame in ogni variante). Il banco ha ora
`--scale N` (predefinita 2): qui gira con `--scale 1`, output 1280×800 pixel. L'output
logico, la geometria e il percorso del puntatore sono gli stessi; i pixel da disegnare
sono un quarto.

Mediana di 3 ripetizioni, tempi in ms, memoria in MiB. Fra parentesi il valore in WSL.

| Variante | Rende con | Avvio | Frame | p95 | p99 | Max | Lenti | PSS |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| flutter-gl | llvmpipe | 1794 (361) | 597 | 19,03 | 20,74 (30,57) | 33,6 | 2 (8) | 113,1 (106,5) |
| flutter-gl-impeller | llvmpipe | — | — | — | — | — | crash | — |
| iced-gl | llvmpipe | 1868 (297) | 597 | 19,14 | 20,89 (21,39) | 35,0 | 3 (0) | 103,3 (60,8) |
| iced-vulkan | lavapipe | 143 | 597 | 19,05 | 20,60 | 32,5 | 2 | 81,3 |
| flutter-software | CPU | 44 (20) | 598 | 19,03 | 20,29 (21,86) | 26,0 | 1 (0) | 25,7 (33,3) |
| iced-software | tiny-skia | 27 (10) | 598 | 19,29 | 21,12 (21,05) | 29,4 | 1 (0) | 11,7 (11,4) |

Frame lenti nelle singole ripetizioni: flutter-gl 2, 2, 2; iced-gl 2, 3, 3; iced-vulkan
3, 2, 2; flutter-software 3, 1, 1; iced-software 1, 1, 1. Avvio delle varianti llvmpipe e
lavapipe molto variabile (113–2072 ms): in alcune ripetizioni il caricatore EGL o Vulkan
prova prima i driver NVIDIA.

Risposte alle tre domande, per quanto queste misure permettono:

- **Flutter con la GPU perde ancora frame?** Non misurabile finché Ossido non offre dmabuf.
  Con llvmpipe flutter-gl perde lo 0,3% dei frame (2 su 597) con p99 20,7 ms, contro
  l'1,4% e 30,6 ms in WSL, ed è ora alla pari con iced-gl (3 frame, p99 20,9 ms). Il
  confronto regge (stesso renderer per entrambi), ma non dice nulla del percorso GPU reale.
  È coerente con l'ipotesi del rapporto: le perdite in WSL venivano dal percorso del
  frame, non dal disegno di Flutter.
- **Impeller cambia qualcosa?** Su llvmpipe l'engine va in segmentation fault durante lo
  scorrimento in 4 tentativi su 4 (`embedder_surface_gl_impeller.cc` dichiara il backend
  OpenGLES, poi il processo muore e il banco non trova più la superficie).
  Il crash è dentro l'engine; non c'è una misura da confrontare.
- **iced su Vulkan è trasparente?** Sì, ma con lavapipe, non con la GPU: lo screenshot
  `iced-vulkan-hover.png` mostra lo sfondo di Ossido attorno al Dock. Anche iced-gl su
  llvmpipe qui è trasparente, a differenza che in WSL. iced-software resta opaco (nero
  attorno al Dock). Con Vulkan su Intel o NVIDIA la domanda resta aperta per la stessa
  ragione: serve dmabuf.

Cosa cambia per il gate 1: le misure native non contraddicono la raccomandazione, ma ne
tolgono l'argomento principale. Il vantaggio di fluidità di iced in WSL non si ripete qui
(sullo stesso renderer le due versioni sono pari); resta il vantaggio di memoria, che in
software è di 14 MiB (25,7 contro 11,7) e con i driver GPU dipende da quale driver si
carica. Una misura con la GPU vera richiede `linux-dmabuf` in Ossido, che serve comunque
(vedi "Cosa resta aperto").

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
- La misura su hardware Linux reale con la GPU: fatta senza GPU il 2026-10-12 (sezione
  sopra); con la GPU dopo dmabuf in Ossido.
- Il limite di memoria per la shell, che fissi tu.

## Ripetere le misure

```bash
SILICA_ENGINE_SRC=~/build/flutter-engine cargo xtask engine --release
cargo xtask bundle dock --release
cargo xtask bench dock                 # --runs N --seconds S --only VARIANTE --scale N
```

Risultati in `target/bench/dock/results.json`, screenshot accanto.

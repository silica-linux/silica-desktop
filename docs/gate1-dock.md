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

Esito delle condizioni (2026-10-13, sezione "Misure con la GPU su hardware nativo"):
dmabuf in Ossido fatto; Impeller con la GPU perde lo 0,17% dei frame; il crash su
llvmpipe è aggirato con il ripiego su Skia. Memoria: decisa il 2026-10-13, si accettano
per ora ~226 MiB di PSS per la shell completa (sfondo, barra, Dock, Faro, un engine per
componente con Impeller); un solo engine con più viste resta un'ottimizzazione possibile.
- Il limite di memoria della shell: vedi l'esito qui sotto (accettati ~226 MiB per ora).

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
  confronto, non come valori assoluti. Le misure su Linux nativo con la GPU sono più sotto.

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

## Misure con la GPU su hardware nativo (2026-10-13)

Ossido ora offre `zwp_linux_dmabuf_v1` versione 4 con feedback sul render node della sua
GPU (`crates/ossido/src/dmabuf.rs`, solo backend winit). I client annidati rendono sulla
GPU di Ossido, la Iris Xe (`renderer OpenGL ES: Mesa Intel(R) Iris(R) Xe Graphics`), e
passano i buffer senza copie. Stesso PC e stesso banco della sezione precedente, con
`--scale 1`. L'embedder ha `--renderer impeller` (OpenGL ES con Impeller), che il banco
usa per flutter-gl-impeller.

Mediana di 3 ripetizioni, tempi in ms, memoria in MiB.

| Variante | Rende con | Avvio | Frame | p95 | p99 | Max | Lenti | PSS | RSS |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| flutter-gl-impeller | Iris Xe | 72 | 598 | 18,30 | 19,23 | 27,5 | 1 | 72,1 | 144,9 |
| flutter-gl | Iris Xe | 79 | 598 | 18,23 | 18,98 | 28,3 | 1 | 60,7 | 132,6 |
| iced-gl | Iris Xe | 79 | 600 | 18,26 | 19,01 | 28,6 | 1 | 50,0 | 122,8 |
| iced-vulkan | Vulkan (GPU scelta da wgpu) | 273 | 598 | 18,05 | 19,75 | 30,5 | 2 | 99,1 | 166,1 |
| flutter-software | CPU | 45 | 597 | 19,10 | 20,70 | 31,5 | 2 | 25,6 | 29,3 |
| iced-software | tiny-skia | 27 | 598 | 19,21 | 21,19 | 25,9 | 1 | 11,7 | 16,0 |

Frame lenti nelle singole ripetizioni: flutter-gl-impeller 0, 1, 1; flutter-gl 1, 1, 1;
iced-gl 0, 1, 2; iced-vulkan 2, 1, 2; flutter-software 2, 2, 3; iced-software 1, 1, 0.
Il frame lento isolato compare in tutte le varianti, anche in software: è del percorso
fra Ossido e GNOME, non dei toolkit.

Condizioni del gate 1:

| Condizione | Esito |
| --- | --- |
| Frame lenti < 1% con la GPU | sì: Impeller 0,17% (1 su 598), p99 19,2 ms, max 31,8 ms |
| Nessun intervallo oltre 50 ms | sì |
| Impeller come renderer | sì, su OpenGL ES; nessun crash con la GPU |
| Memoria | Dock con Impeller: 72 MiB di PSS (Skia 61, iced 50). Il limite lo fissi tu |

Con la GPU vera il ritardo di Flutter visto in WSL scompare: le tre varianti GPU hanno lo
stesso ritmo. Impeller costa circa 11 MiB più di Skia. Lo screenshot
`flutter-gl-impeller-hover.png` ha la stessa geometria degli altri e la superficie è
trasparente.

Impeller su un driver GL software (llvmpipe) va ancora in crash dentro l'engine: con
`--renderer impeller` l'embedder ora riconosce il driver e ripiega su Skia, con un
avviso nel log. Il test `impeller_renderer_draws_and_reacts_to_clicks` copre il caso
(in headless e in CI il driver è llvmpipe). Impeller su Vulkan non è misurato: l'embedder
non ha ancora un renderer Vulkan, e su OpenGL ES Impeller soddisfa già le condizioni.

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

## Impeller su Vulkan (2026-10-13)

`--renderer vulkan`: l'embedder crea istanza e dispositivo Vulkan (`ash`), una swapchain
`VK_KHR_wayland_surface` sulla superficie dell'app con alfa premoltiplicata, presentazione
MAILBOX o IMMEDIATE (il ritmo resta il vsync dell'engine), e dà tutto all'engine con
`FlutterVulkanRendererConfig` e `--enable-impeller`. Preferisce la GPU integrata
(`SILICA_VULKAN_GPU=discrete` per la discreta); senza Vulkan ripiega su OpenGL ES con Impeller.
È il renderer della shell in `cargo xtask run --shell`.

Perché: con Impeller su OpenGL ES questo driver non dà il multisampling e i tracciati
(glifi, contorni) restano senza antialiasing; su Vulkan sì, con la stessa qualità di Skia.

Misura: Ossido annidato in GNOME (XPS 15, Iris Xe, Mesa), Dock della shell 1600 × 1000, il
puntatore avanti e indietro sul Dock per ~6 s (`perf.frames`, `perf.process`), due ripetizioni:

| Renderer | Frame | p50 | p95 | p99 | Max | Lenti | PSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Skia (GL) | 431 | 16,7 | 20,1 | 31,6 | 43,7 | 7 | 75 MiB |
| Impeller (GL) | 423 | 16,7 | 19,0 | 20,7 | 29,5 | 1 | 80 MiB |
| Impeller (Vulkan) | 427–429 | 16,7 | 19,0–19,4 | 20,0–20,6 | 23–29 | 0–1 | 78 MiB |

Una prima ripetizione con Impeller su GL ha avuto un intoppo di 3 s (102 frame), non
ripetuto. Vulkan è pari o meglio su tutto e ha l'antialiasing.

## Cosa resta aperto

- `wp_presentation` per il ritmo dei frame (dmabuf in Ossido è fatto).
- Impeller su Vulkan in una sessione reale (DRM/KMS) e sulla GPU discreta.
- La misura in una sessione reale (DRM/KMS, M6), non annidata in GNOME.
- Un solo engine con più viste per la shell, se la memoria diventa un problema.

## Ripetere le misure

```bash
SILICA_ENGINE_SRC=~/build/flutter-engine cargo xtask engine --release
cargo xtask bundle dock --release
cargo xtask bench dock                 # --runs N --seconds S --only VARIANTE --scale N
```

Risultati in `target/bench/dock/results.json`, screenshot accanto.

# Roadmap

Dieci milestone in tre fasi, con due gate che decidono se proseguire: l'embedder dopo
M1 e l'uso quotidiano dopo M6. Le date arrivano solo dopo il gate 1: fino ad allora il
costo dell'embedder è l'incognita che pesa su tutto il resto.

## Fondamenta (M0 – M2)

### M0 · Fondamenta

- Workspace Cargo, `cargo xtask ci`, `AGENTS.md`, `ROADMAP.md`
- Ossido annidato mostra finestre xdg-shell
- Canale di controllo con screenshot, test in headless

### M1 · Embedder Flutter

- App Flutter su Ossido tramite embedder Rust: finestre, input, HiDPI
- Poi layer-shell per la shell e stima del costo di manutenzione
- Renderer OpenGL e prototipo del Dock in Flutter e in Rust (iced), misurati sullo stesso
  banco di prova (`docs/m1.5-dock.md`)

### Gate 1 · toolkit della shell

Si sceglie dalle misure del prototipo: shell e app in Flutter con l'embedder custom,
shell in Rust e app in Flutter, oppure embedder GTK ufficiale come ripiego.

Deciso il 2026-10-13: shell e app in Flutter con l'embedder custom e Impeller
(`docs/gate1-dock.md`).
Prima di M2: `linux-dmabuf` in Ossido e una misura del Dock con la GPU su hardware nativo.

### M2 · Vetro e finestre

- Shader di sfocatura e rifrazione, protocollo `silica-glass-v1`
- Decorazioni lato server, angoli, ombre, animazioni di apertura e chiusura

## Shell (M3 – M5)

### M3 · Shell base

- Barra dei menu con orologio e stato, Dock, Faro sui file `.desktop`, sfondo

### M4 · Servizi

- `silica-daemon`: server delle notifiche e Centro notifiche
- Centro di controllo: rete, Bluetooth, audio, luminosità, batteria

### M5 · Panorama

- Vista di tutte le finestre, spazi di lavoro, gesture del touchpad a 3 e 4 dita

## Sessione e app (M6 – M9)

### M6 · Sessione reale

- DRM/KMS, libinput, logind, lock screen, login con greetd
- XWayland, multi-monitor, scala frazionaria, test su hardware reale

### Gate 2 · uso quotidiano

Silica diventa la sessione di sviluppo: si prosegue se regge una settimana.

### M7 · Integrazione app

- Menu globale: `silica-menu-v1` e ponte DBusMenu per Qt e GTK3
- Portal: cattura schermo e selettore file; tema GTK e Qt approssimato

### M8 · App

- Impostazioni e Mustache in Flutter, sopra `silica_ui`

### M9 · Alpha 0.1

- Pacchetti per Ubuntu, Fedora e Arch, primo nome di versione
- Parità misurata pubblicata in questo file

## Parità misurata

Si aggiorna solo da metriche misurate (differenza visiva sugli screenshot, test di
conformità), mai stimata.

| Area | Metrica | Valore | Misurato il |
| --- | --- | --- | --- |
| Finestre · angoli | pixel mancanti per riga dell'angolo contro la maschera di riferimento (test `window_corners_follow_the_reference`) | errore massimo 0,44 px, rms 0,13 px | 2026-10-13 |
| Finestre · ombra | alfa del profilo laterale (d 2–48 pt), anello e asimmetria sopra/sotto, finestra attiva e inattiva in chiaro (test `window_shadows_follow_the_reference`) | errore massimo 2 livelli d'alfa su 255 | 2026-10-13 |
| Vetro · colore | riempimento su `#7a7b80`, materiale chiaro (Dock) e spesso (menu), aspetto chiaro (test `materials_on_the_reference_gray_match_the_measured_fill`) | entro 3 livelli su 255 per canale | 2026-10-13 |
| Vetro · sfocatura | profilo di un bordo di striscia nel materiale chiaro contro il Dock (test `clear_glass_blurs_a_stripe_edge_like_the_dock`) | rms 5,0 livelli su 255 | 2026-10-13 |
| Vetro · bordo | riga luminosa, sfumatura di 7 righe e bordo laterale del materiale chiaro su `#7a7b80` contro il Dock (test `clear_glass_edge_matches_the_dock_border`) | entro 3 livelli su 255 per canale | 2026-10-13 |
| Vetro · rifrazione | spostamento degli attraversamenti di strisce vicino al bordo laterale, materiale chiaro (test `clear_glass_refracts_near_the_side_edges_like_the_dock`) | 4,6 pt a 5,4 pt dal bordo (Dock: ~5 pt a 6 pt), 0 oltre 22 pt | 2026-10-13 |
| Liquid Glass · lente | sorgente prelevata in funzione della distanza dal bordo, materiale `regular`, contro la curva misurata sui moduli del Centro di controllo (test `regular_glass_is_the_liquid_lens_of_the_control_center`) | entro 1,1 pt nei 30 pt dal bordo (incertezza della misura ±3 pt) | 2026-10-13 |
| Liquid Glass · colore | riempimento del materiale `regular` su `#7a7b80` contro i moduli (`#6a6b70`) | entro 2 livelli su 255 | 2026-10-13 |
| Dock · geometria | screenshot di Ossido headless a 2560×1440 su `#7a7b80` contro `shots/dock/rest-light.png` e `hover-mail-light.png`: bordi del fondo a riposo e ingrandito, etichetta, indicatori (test dei numeri in `shell/test/dock_model_test.dart`) | bordi entro 1 pt, etichetta entro 1 pt | 2026-10-13 |
| Dock · fondo | profilo verticale del fondo (riga luminosa, sfumatura, riempimento) nella colonna fra due icone, stesso confronto | entro 2 livelli su 255 | 2026-10-13 |
| Barra dei menu | screenshot a 2560×1440 su `#7a7b80` contro `shots/glass/menubar-gray-light.png`: fondale (gradiente sotto la barra), righe dell'inchiostro del testo, bordo destro dell'orologio, logo | fondale entro 1 livello; testo sulle stesse righe (9–18) e stessa x di partenza; orologio allo stesso bordo; logo 1 pt più in basso | 2026-10-13 |
| Notifiche · vetro | riempimento del materiale `notification` su `#7a7b80` contro il banner isolato `#c0c1c6`; due carte a 6 pt restano separate (test `notification_glass_matches_the_isolated_banner`) | entro 3 livelli su 255 | 2026-10-13 |
| Notifiche · banner | screenshot a 2560×1440 su `#7a7b80` con la barra dei menu contro `shots/notifications/banner-gray-light.png`: cornice, riempimento (colonna 2530), bordo sopra, sotto e ai lati, maiuscola del titolo, ombra sotto | cornice x 2200–2544, y 46, raggio 20 come il riferimento; riempimento entro 2 livelli; bordo entro 3 (sotto 9); titolo alla stessa riga (61) e stessa x (2258); ombra entro 1 livello | 2026-10-13 |
| Centro di controllo · griglia | screenshot a 2560×1440 su `#7a7b80` con la barra contro `shots/controlcenter/cc-gray-light-first.png`: righe luminose di ogni modulo, bordi laterali, disco attivo del Wi-Fi; nessun ponte fra moduli a 12 pt (test `every_region_of_a_group_has_its_rim_on_its_own_edge`) | righe e bordi sugli stessi pixel (y 42, 118, 194, 270, 346; x 2253 e 2394); disco sugli stessi pixel | 2026-10-13 |
| Centro di controllo · vetro | stesso confronto: riempimento, riga luminosa e sfumatura sopra e sotto, lati | riempimento entro 1 livello; righe entro 6 (sopra) e 6 (sotto); lati entro 1; sfumatura entro 8 | 2026-10-13 |
| Centro di controllo · ombra | profilo a sinistra della pila (0–160 pt), a destra e sopra, stesso confronto | entro 2 livelli su 255 | 2026-10-13 |
| HUD del volume | screenshot a 2560×1440 su `#7a7b80` con la barra contro `shots/misc/volume-hud-light.png`: righe luminose, bordi laterali, traccia, fine del riempimento sulla tacca del livello, titolo | righe e bordi sugli stessi pixel (y 40 e 103, x 2257 e 2550); traccia da 2290 (rif. 2289); titolo x 2275 (2274) e maiuscola 1 pt più in alto; riempimento entro 8 livelli, riga luminosa superiore 19 sotto | 2026-10-13 |
| Notifiche · comportamenti | curve dell'entrata, dell'uscita e del Centro contro le misure fotogramma per fotogramma della sessione 8 (test `uscita e Centro seguono le misure della sessione 8` e dell'entrata) | entrata: residuo ×0,83 a fotogramma come misurato; uscita: gli stessi spostamenti delle misure; Centro: bordo entro 5 pt a 35 ms, chiusura sugli stessi punti | 2026-10-13 |

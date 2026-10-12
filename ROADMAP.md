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
| — | — | nessuna misura ancora | — |

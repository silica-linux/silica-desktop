# Protocolli Silica

Gli XML sono in `crates/protocols/xml/`; i binding Rust in `silica-protocols` (feature
`server` per Ossido, `client` per l'embedder e i test).

## silica-glass-v1

Un client dichiara regioni di una sua superficie da rendere come vetro: Ossido campiona
quello che sta dietro la superficie e lo mostra sfocato, con il colore e la rifrazione
del materiale. Il client lascia trasparenti i pixel dove deve vedersi il vetro e disegna
sopra testo e icone.

- `silica_glass_manager_v1.get_glass(id, surface)`: un oggetto vetro per superficie
  (un secondo è l'errore `already_has_glass`).
- `silica_glass_v1.clear()`, `add_region(x, y, width, height, radius, material)`: stato
  in attesa, corrente al prossimo `wl_surface.commit`. Coordinate della superficie in
  pixel logici; angoli ad arco di cerchio di raggio `radius`, come i vetri misurati
  (Dock, menu, notifiche: le finestre invece hanno angoli a superellisse).
- Materiali: `clear` (vetro chiaro, come il Dock), `thick` (vetro spesso, come menu e
  ricerca) e `regular` (il Liquid Glass dei controlli, come i moduli del Centro di
  controllo: lente forte che sul bordo preleva anche lo sfondo esterno), dalle misure in
  `plan/reference/glass.md`.
- Errori: `invalid_region` (dimensione non positiva, raggio negativo o non finito,
  materiale sconosciuto), `too_many_regions` (oltre 64). Il client viene disconnesso;
  Ossido continua.

Le regioni correnti si leggono con `glass.list` (`docs/control.md`).

## silica-shell-v1 e l'elenco delle finestre

Riservati ai componenti della shell: processi con `SILICA_SHELL=1` nell'ambiente, che
Ossido mette quando li lancia (`--spawn-shell`, `app.launch` con `shell: true`). Ossido lo
legge alla connessione (pid da `SO_PEERCRED`, ambiente da `/proc`). Non è una barriera
contro altri processi dello stesso utente: quella arriverà con la sandbox delle app.

- `ext-foreign-toplevel-list-v1` (standard): le finestre aperte, con titolo, app id e un
  identificatore.
- `silica_shell_manager_v1.activate(identifier)`: focus alla finestra (e la riporta se è
  ridotta).
- `silica_shell_manager_v1.launch(command)`: avvia un'app come figlia di Ossido.

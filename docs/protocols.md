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
  pixel logici; angoli a superellisse come le finestre.
- Materiali: `clear` (vetro chiaro, come il Dock) e `thick` (vetro spesso, come menu e
  ricerca), dalle due famiglie misurate in `plan/reference/glass.md`.
- Errori: `invalid_region` (dimensione non positiva, raggio negativo o non finito,
  materiale sconosciuto), `too_many_regions` (oltre 64). Il client viene disconnesso;
  Ossido continua.

Le regioni correnti si leggono con `glass.list` (`docs/control.md`).

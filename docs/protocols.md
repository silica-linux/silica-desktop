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
  `plan/reference/glass.md`. Dalla versione 2 anche `notification` (banner e Centro
  notifiche: sfocatura molto ampia, quasi opaco, riga scura del bordo sul primo pixel della
  regione, che il client allarga di 1 pt per lato; le regioni non si fondono). Un client
  legato alla versione 1 che lo chiede è un errore `invalid_region`; l'embedder lì ripiega
  su `thick`.
- Errori: `invalid_region` (dimensione non positiva, raggio negativo o non finito,
  materiale sconosciuto), `too_many_regions` (oltre 64). Il client viene disconnesso;
  Ossido continua.

- Dalla versione 3: `add_translucent_region(…, opacity)`, la stessa regione composta con
  un'opacità 0–1. Un pannello che compare o scompare in dissolvenza porta con sé il vetro;
  l'opacità non rifà la sfocatura (cambia solo il disegno). Fuori da 0–1 è
  `invalid_region`. `silica_ui` la ricava dagli antenati `Opacity` del widget.

- Dalla versione 4: `separate()` chiude il gruppo corrente. Le regioni seguenti non si
  fondono con quelle di prima, anche sovrapposte, e si disegnano sopra (un sottomenu sul
  suo menu: in `menubar.md` sono due pannelli, ognuno con il suo bordo e l'ombra del
  sottomenu sul menu). `clear` riporta al primo gruppo. `add_tip(x, y, width, height,
  material, opacity)`: una punta rivolta in basso, con la base sul lato superiore del
  rettangolo e il vertice arrotondato al centro del lato inferiore, che si fonde con il
  corpo del suo gruppo in una sola forma: il bordo corre continuo dal corpo alla punta
  (la punta dei menu del Dock, `misc.md`). `silica_ui` li manda con `Glass.group` e
  `Glass.tip`; un embedder legato a una versione precedente ignora gruppi e punte.
- Dalla versione 5: `add_upward_tip(...)`, la stessa punta capovolta (base sul lato inferiore,
  vertice in alto): il popover di disposizione sotto il pulsante verde. `GlassTip.up` in
  `silica_ui`.

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
- `silica_shell_manager_v1.execute(id)`: esegue un comando senza parametri del catalogo
  (la lente della barra apre Faro con `faro.toggle`).
- evento `silica_shell_manager_v1.command(id)`: un comando del catalogo che tocca alla
  shell (`faro.toggle`, Logo+Spazio), inoltrato a tutti i componenti.
- evento `silica_shell_manager_v1.focused(identifier)`: la finestra con il focus (vuoto se
  nessuna), al collegamento e a ogni cambiamento.
- evento `silica_shell_manager_v1.window_menu(identifier, x, y)` (versione 3): pressione
  lunga sul pulsante verde (provvisoria: 0,8 s, W-08); la barra dei menu apre il popover di
  disposizione con la punta in x, y, e le voci eseguono `window.tile_*`, `window.arrange_*`
  e `window.fullscreen` sulla finestra con il focus.
- `silica_shell_manager_v1.broadcast(topic, value)` ed evento omonimo (versione 4): uno stato
  di un componente per gli altri (`controlcenter.open`, `notifications.open`: le capsule
  nella barra). Ossido lo rimanda a tutti, mittente compreso, e manda l'ultimo valore di ogni
  argomento a chi si collega dopo.
- `silica_shell_manager_v1.execute_with(id, params)` (versione 5): come `execute`, con i
  parametri in un oggetto JSON (le istantanee: `screenshot.region`, `screenshot.window_at`).
  Ossido annuncia ogni istantanea con il broadcast `screenshot` = "<percorso> <l> <a>".

## Trascinamento fra app (`wl_data_device`)

Protocollo standard, nessuna estensione. Ossido disegna l'icona del trascinamento
(`start_drag`) con l'angolo sul puntatore, sopra tutto, fino al rilascio.

L'embedder lo offre alle app sul canale `silica/drag` (JSONMethodCodec, `SilicaDrag` in
silica_ui ≥ 0.16):

- `start {uris, icon?: {width, height, rgba}}` dall'app: una sorgente `text/uri-list`
  (azioni copia e sposta) con il seriale dell'ultima pressione. Quando Ossido prende il
  puntatore, il gesto di Flutter in corso riceve un annullamento.
- Verso l'app: `enter`/`motion {x, y}` e `leave` per i trascinamenti con `text/uri-list`
  sopra la finestra (accettati, copia preferita), `drop {x, y, uris}` con i dati già letti
  (su un thread: la sorgente può essere la finestra stessa), `finished {action}` per quelli
  usciti dalla finestra.

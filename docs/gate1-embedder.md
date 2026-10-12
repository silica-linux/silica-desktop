# Gate 1 · Embedder Flutter custom

> Aggiornamento M1.5: il confronto misurato fra Flutter e Rust per la shell è in
> `docs/gate1-dock.md`, che sostituisce la raccomandazione qui sotto per la shell.
> Decisione del 2026-10-13: shell e app in Flutter.
> Il renderer OpenGL e l'engine release (rischi aperti qui sotto) ora ci sono.

Raccomandazione: **go**. L'embedder custom in Rust porta un'app Flutter su Ossido con
rendering, puntatore, tastiera, testo, HiDPI e layer-shell in circa 2.400 righe scritte a
mano, con l'API dell'engine stabile e l'aggiornamento di versione automatizzato. Due
rischi restano aperti e vanno misurati prima di M3: il renderer GPU e il protocollo
della tastiera.

La decisione spetta a te (AGENTS.md, "Inizio sessione"). Dati al 2026-10-12, Flutter
3.47.7, engine `deb287481e3ce9468f3434937ced4240a70539ca`.

## Cosa funziona, verificato da test

Ogni riga ha un test che gira in `cargo xtask ci`, senza GPU, su Ossido headless.

| Capacità | Come | Test |
| --- | --- | --- |
| Primo frame | renderer software dell'engine → `wl_shm` | `demo_renders_reacts_to_clicks_and_closes` |
| Puntatore | `FlutterPointerEvent` (add, hover, down, move, up, scroll) | stesso test: clic → colore cambia |
| HiDPI | `wl_surface` buffer scale + `pixel_ratio` | `demo_renders_at_scale_two` |
| Tasti | `FlutterKeyEvent` + messaggio GTK su `flutter/keyevent` | `keyboard_reaches_text_field_and_hardware_keyboard` |
| Testo | `flutter/textinput` con modello di editing (UTF-16) | stesso test + 5 test unitari |
| Layer-shell | `zwlr_layer_surface_v1`: ancore, zona esclusiva, tastiera esclusiva | `layer_bar_reserves_space_takes_keyboard_and_clicks` |
| Chiusura | `FlutterEngineShutdown`, uscita pulita | `demo_renders_reacts_to_clicks_and_closes` |

Verificato anche a occhio, annidato in WSLg: app in finestra, barra in alto con finestra
sotto.

## Quanto costa mantenerlo

| Voce | Misura |
| --- | --- |
| Codice scritto a mano | 2.430 righe in `crates/embedder/src` (test unitari inclusi), più 378 di test e2e |
| Codice generato | 3.027 righe di binding (`cargo xtask bindings`), 427 di tabelle tasti (`cargo xtask keymap`) |
| API dell'engine usata | 11 funzioni (sotto) |
| `unsafe` | 28 blocchi, tutti in `src/ffi/mod.rs`, ognuno con `SAFETY` |
| Engine | `libflutter_engine.so` 43 MB, caricata a runtime |

Funzioni usate: `FlutterEngineRun`, `FlutterEngineShutdown`, `FlutterEngineRunTask`,
`FlutterEngineGetCurrentTime`, `FlutterEngineSendWindowMetricsEvent`,
`FlutterEngineSendPointerEvent`, `FlutterEngineSendKeyEvent`,
`FlutterEngineSendPlatformMessage`, `FlutterEngineSendPlatformMessageResponse`,
`FlutterPlatformMessageCreateResponseHandle`, `FlutterPlatformMessageReleaseResponseHandle`.

**Aggiornare Flutter.** I bundle girano solo sull'engine dello stesso SDK, quindi a ogni
aggiornamento:

1. aggiornare `crates/embedder/ENGINE_VERSION` (`cargo xtask engine` rifiuta versioni diverse dall'SDK);
2. `cargo xtask engine && cargo xtask bindings && cargo xtask keymap`;
3. `cargo xtask ci`: i test e2e dicono se qualcosa si è rotto.

`flutter_embedder.h` è stabile per costruzione: ogni struct porta `struct_size` e
`FLUTTER_ENGINE_VERSION` è 1 da quando esiste. Il rischio non è l'ABI, sono i protocolli
dei canali (sotto).

## Rischi aperti

| Rischio | Impatto | Cosa fare |
| --- | --- | --- |
| Solo renderer software: costo per frame non misurato; a 4K o con animazioni può non bastare | Alto | M1.5: renderer OpenGL (EGL + `wl_egl_window` o dmabuf) e misura del tempo per frame a 1280×800 e 2560×1600 |
| Tastiera: il framework accoda i `KeyEvent` finché non riceve il messaggio grezzo `flutter/keyevent`, deprecato dal 3.18. Quando lo toglieranno il protocollo cambia | Medio | Seguire i cambi dell'embedder GTK ufficiale a ogni aggiornamento; il test e2e lo segnala subito |
| Funzioni dell'embedder GTK non ancora rifatte | Medio | Elenco sotto; ognuna è un canale o un protocollo Wayland, piccola da sola |

Ancora da fare rispetto all'embedder GTK ufficiale, con una stima a occhio:

- Appunti (`flutter/platform` Clipboard) e cursori (`flutter/mousecursor`): 1 giorno.
- Ripetizione dei tasti tenuti premuti: mezza giornata.
- IME (`text-input-v3`) per lingue con composizione: 2-3 giorni.
- Accessibilità (semantica → AT-SPI): 1-2 settimane, la voce più grossa.
- Scala frazionaria (`wp_fractional_scale_v1`): 1 giorno.
- Popup e menu come superfici xdg_popup: 2-3 giorni, serve per la barra dei menu (M3).

## L'alternativa no-go

Embedder GTK ufficiale (`libflutter_linux_gtk.so`) con `gtk-layer-shell` per barra e Dock.
Funziona oggi, ma il vetro di Silica passa da un protocollo Wayland proprio
(`silica-glass-v1`): con GTK serve un plugin che recuperi la `wl_surface` da GDK, e il
controllo su frame, input e scala resta a GTK. Si perde l'harness headless: i test della
shell richiederebbero un display.

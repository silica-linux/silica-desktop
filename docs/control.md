# Canale di controllo

Ossido apre il canale con `--control PORTA` su `127.0.0.1` (`0` sceglie una porta
libera) e annuncia l'indirizzo su stdout con una riga `OSSIDO_CONTROL=127.0.0.1:PORTA`.
Il canale serve agli agenti e ai test: non è autenticato, quindi resta su localhost.

## Protocollo

Una riga JSON per messaggio. Il client manda una richiesta, Ossido risponde con lo
stesso `id`. Le richieste sulla stessa connessione sono servite in ordine.

```
→ {"id":1,"command":"window.move","params":{"x":10,"y":20}}
← {"id":1,"ok":true,"result":null}
← {"id":2,"ok":false,"error":"nessuna finestra con id 9"}
```

Una richiesta oltre 1 MiB chiude la connessione. Parametri sconosciuti sono un errore.

## Comandi

Il catalogo è in `crates/control/src/commands.rs`; `engine.commands` lo restituisce con
gli schemi dei parametri. Ogni id del catalogo è eseguito da Ossido (test
`every_catalog_command_is_implemented`).

| Comando | Parametri | Risultato |
| --- | --- | --- |
| `ui.screenshot` | `path?` | `width`, `height`, `path` o `png_base64` |
| `ui.animation_scale` | `scale` (0–1000) | — (moltiplica le durate delle animazioni; 0 le spegne) |
| `ui.glass_quality` | `quality` (`auto`, `full`, `frosted`, `opaque`) | `quality` in uso |
| `engine.commands` | — | il catalogo |
| `engine.execute` | `id`, `params?` | il risultato del comando |
| `window.list` | — | finestre dall'alto in basso, poi le ridotte: `id`, `title`, `app_id`, `x`, `y`, `width`, `height` (del contenuto, senza la barra), `focused`, `minimized`, `fullscreen`, `space` |
| `input.gesture` | `kind` (`swipe`, `pinch`), `fingers`, `phase?` (`begin`, `update`, `end`; senza: tutto il gesto), `dx`, `dy`, `scale`, `cancelled` | `command` eseguito alla fine (null se nessuno). Provvisorio (PA-20…22): 3–4 dita ←/→ spazio, ↑ Panorama, ↓ finestre dell'app o chiude la vista; pollice e tre dita: allargando la scrivania, stringendo il launcher |
| `space.list` | — | spazi: `index`, `active`, `windows` (id dal basso verso l'alto) |
| `space.next`, `space.previous` | — | `switched`: falso oltre l'ultimo o prima del primo (nessun rimbalzo); ⌃→ e ⌃← |
| `space.goto` | `index` | `switched` |
| `space.add` | — | `index` dello spazio nuovo (al più 16) |
| `space.remove` | `index` | — ; le finestre passano allo spazio prima (o al primo) |
| `window.move_to_space` | `id?`, `index` | — |
| `panorama.toggle` | — | `open`: apre o chiude Panorama (⌃↑; Esc chiude). Aperto, un clic su una finestra la porta in primo piano, sul vuoto chiude, su uno spazio della barra ci passa, sul "+" aggiunge uno spazio |
| `panorama.state` | — | `mode` (`panorama`, `app`, `desktop`; assente se chiuso), `open`, `progress` (0–1), `windows`: `id`, `x`, `y`, `width`, `height` delle cornici disposte; `expansion` della barra degli spazi (0–1), `thumbnails`: rettangoli delle miniature quando si vedono |
| `panorama.app` | — | `open`: App Exposé, le finestre dell'app in primo piano al centro con il titolo sotto (⌃↓; Esc chiude) |
| `desktop.show` | — | `open`: Mostra scrivania, le finestre escono verso il bordo più vicino (F11; di nuovo o un clic le rimette a posto) |
| `panorama.spaces_bar` | `enabled` | — ; espande o richiude la barra degli spazi (come il puntatore in alto). Con la barra espansa una finestra trascinata su una miniatura passa a quello spazio; un clic su una miniatura ci va |
| `switcher.next`, `switcher.previous` | — | `forwarded`: apre il selettore delle app o sposta la scelta (Logo+Tab, Logo+Shift+Tab); finché è aperto il rilascio del tasto Logo esegue `switcher.commit` ed Esc `switcher.cancel` |
| `switcher.commit`, `switcher.cancel` | — | `forwarded`: chiude il selettore attivando l'app scelta, o senza cambiare app |
| `launcher.toggle` | — | `forwarded`: apre o chiude il launcher delle app (Faro in modalità griglia) |
| `faro.toggle` | — | `forwarded`: componenti della shell che l'hanno ricevuto (Logo+Spazio) |
| `notifications.toggle` | — | `forwarded`: apre o chiude il Centro notifiche (clic sull'orologio) |
| `controlcenter.toggle` | — | `forwarded`: apre o chiude il Centro di controllo (icona nella barra) |
| `system.status` | — | stato da silica-daemon (`docs/daemon.md`, System1): `wifi.*`, `bluetooth.*`, `battery.*`, `brightness`, `audio.*`, `dnd`; errore se il daemon non risponde |
| `system.wifi`, `system.bluetooth`, `system.mute`, `system.dnd` | `enabled` | `queued`: la chiamata parte su un thread, senza attendere il daemon |
| `system.volume`, `system.brightness` | `value` (0–1) | `queued` |
| `system.volume_up`, `system.volume_down`, `system.mute_toggle` | — | `queued`; passi di 1/16, alzare toglie il muto (tasti volume e muto) |
| `system.brightness_up`, `system.brightness_down` | — | `queued`; passi di 1/16 (tasti luminosità) |
| `system.volume_up_fine`, `system.volume_down_fine`, `system.brightness_up_fine`, `system.brightness_down_fine` | — | `queued`; un quarto di passo, 1/64 (⌥⇧ con i tasti) |
| `glass.list` | — | superfici con vetro (`silica-glass-v1`): `surface`, `x`, `y`, `regions` (`x`, `y`, `width`, `height`, `radius`, `material`, `opacity`) |
| `layer.list` | — | layer della shell: `namespace`, `layer`, `x`, `y`, `width`, `height`, `exclusive_zone`, `focused` |
| `window.focus` | `id?` | — (riporta anche una finestra ridotta) |
| `window.close` | `id?` | — |
| `window.move` | `id?`, `x`, `y` | — |
| `window.minimize` | `id?` | — (Logo+M) |
| `window.fullscreen` | `id?` | — (Logo+Ctrl+F; di nuovo per tornare) |
| `app.launch` | `command`, `shell?` | `pid` (con `shell` il processo vede i protocolli della shell) |
| `session.quit` | — | — |
| `input.pointer_move` | `x`, `y` | — |
| `input.pointer_button` | `button?`, `pressed` | — |
| `input.click` | `x`, `y`, `button?` | — |
| `input.key` | `key` (nome xkb o carattere), `pressed?` | — ; come un tasto vero, anche le scorciatoie del catalogo (⌃→ cambia spazio) |
| `input.type` | `text` | — |
| `perf.frames` | `reset?` | per superficie: `surface`, `pid`, `frames`, `mean_ms`, `p50_ms`, `p95_ms`, `p99_ms`, `max_ms`, `slow`, `period_ms`, `startup_ms` |
| `perf.process` | `pid` | `pid`, `rss_kib`, `pss_kib` |
| `perf.render` | `reset?` | rendering di Ossido (winit): `frames`, `mean_ms`, `p50_ms`, `p95_ms`, `p99_ms`, `max_ms` |

Le misure di `perf.frames` sono quelle del compositor, uguali per ogni toolkit: un frame è
un commit con un buffer nuovo, l'intervallo è il tempo fra due frame della stessa
superficie, lento se supera 1,5 periodi di refresh dell'output. `startup_ms` va dal lancio
con `app.launch` (o `--spawn`) al primo buffer del processo o di un suo discendente.

Senza `id`, i comandi sulle finestre agiscono su quella con il focus.

## CLI e MCP

```
cargo xtask run --nested --control 7979 --spawn foot
cargo xtask run --nested --shell --control 7979   # con sfondo, barra dei menu e Dock
silica-cli window.list
silica-cli window.move x=100 y=80
silica-cli screenshot /tmp/silica.png
silica-cli mcp            # server MCP su stdio, configurato in .mcp.json
```

Via MCP i comandi sono tool con `_` al posto di `.` (`window_list`); `ui_screenshot`
senza `path` restituisce l'immagine come contenuto `image`.

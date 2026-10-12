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
| `engine.commands` | — | il catalogo |
| `engine.execute` | `id`, `params?` | il risultato del comando |
| `window.list` | — | finestre dall'alto in basso, poi le ridotte: `id`, `title`, `app_id`, `x`, `y`, `width`, `height` (del contenuto, senza la barra), `focused`, `minimized`, `fullscreen` |
| `glass.list` | — | superfici con vetro (`silica-glass-v1`): `surface`, `x`, `y`, `regions` (`x`, `y`, `width`, `height`, `radius`, `material`) |
| `layer.list` | — | layer della shell: `namespace`, `layer`, `x`, `y`, `width`, `height`, `exclusive_zone`, `focused` |
| `window.focus` | `id?` | — (riporta anche una finestra ridotta) |
| `window.close` | `id?` | — |
| `window.move` | `id?`, `x`, `y` | — |
| `window.minimize` | `id?` | — (Logo+M) |
| `window.fullscreen` | `id?` | — (Logo+Ctrl+F; di nuovo per tornare) |
| `app.launch` | `command` | `pid` |
| `session.quit` | — | — |
| `input.pointer_move` | `x`, `y` | — |
| `input.pointer_button` | `button?`, `pressed` | — |
| `input.click` | `x`, `y`, `button?` | — |
| `input.key` | `key` (nome xkb o carattere), `pressed?` | — |
| `input.type` | `text` | — |
| `perf.frames` | `reset?` | per superficie: `surface`, `pid`, `frames`, `mean_ms`, `p50_ms`, `p95_ms`, `p99_ms`, `max_ms`, `slow`, `period_ms`, `startup_ms` |
| `perf.process` | `pid` | `pid`, `rss_kib`, `pss_kib` |

Le misure di `perf.frames` sono quelle del compositor, uguali per ogni toolkit: un frame è
un commit con un buffer nuovo, l'intervallo è il tempo fra due frame della stessa
superficie, lento se supera 1,5 periodi di refresh dell'output. `startup_ms` va dal lancio
con `app.launch` (o `--spawn`) al primo buffer del processo o di un suo discendente.

Senza `id`, i comandi sulle finestre agiscono su quella con il focus.

## CLI e MCP

```
cargo xtask run --nested --control 7979 --spawn foot
silica-cli window.list
silica-cli window.move x=100 y=80
silica-cli screenshot /tmp/silica.png
silica-cli mcp            # server MCP su stdio, configurato in .mcp.json
```

Via MCP i comandi sono tool con `_` al posto di `.` (`window_list`); `ui_screenshot`
senza `path` restituisce l'immagine come contenuto `image`.

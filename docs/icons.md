# Icone delle app nella shell

Le icone vengono dal repo `silica-linux/silica-icons` (CC BY-SA 4.0), che non è
incluso in questo repository: la shell le legge da disco. Il formato è descritto in
`docs/icon-format.md` di quel repo.

## Dove le cerca

In ordine, la prima che contiene `apps/<id>/icon.json` o `files/<id>/icon.json` vince:

1. `$SILICA_ICONS_DIR`: per lo sviluppo, il clone del repo. `cargo xtask run` lo imposta da
   solo se `silica-icons` è clonato accanto a questo repository (`~/silica/silica-icons`);
2. `silica/icons` nelle cartelle dei dati XDG: `~/.local/share/silica/icons`, poi
   `/usr/local/share/silica/icons` e `/usr/share/silica/icons`.

Codice: `shell/lib/system/silica_icons.dart`.

## Quale icona spetta a un'app

1. La chiave `X-Silica-Icon=<id>` nella voce `.desktop` dell'app.
2. La tabella `knownApps`: le app GNOME più comuni e i luoghi del Dock (Download →
   `folder`, Cestino → `trash-empty` o `trash-full`).
3. Altrimenti l'icona del tema freedesktop (`icon_lookup.dart`), ridotta al keyline 600
   su un fondo neutro con la stessa forma (`FlatSource`).
4. Senza nessuna icona, la tessera con l'iniziale (`dock/icons.dart`).

## Come la disegna

`shell/lib/system/shell_icon.dart` (`ShellIcon`):

- il corpo visibile è il lato chiesto (48 pt nel Dock, 38 pt in Faro); la tela 1024
  sborda di 100/824 per lato;
- maschera a superellisse per angolo, n 2,6, raggio 0,32 × lato
  (`plan/reference/icons.md`); bordo luminoso sfumato e ombra sotto il corpo;
- ombra neutra sotto i livelli con `"shadow": "neutral"`;
- tema scuro dai file `-dark`, scelto dalla luminosità della piattaforma;
- orologio: lancette ruotate ogni secondo (i secondi da 64 pt), ogni 20 s sotto;
  calendario: mese in italiano e giorno, aggiornati a mezzanotte.

## Limiti degli SVG

flutter_svg non applica i filtri SVG. Al caricamento la shell:

- toglie le copie dei tracciati che servono solo alle ombre (`<use filter=…>`);
- trasforma ellissi e cerchi sfocati in un gradiente radiale;
- ignora gli altri `filter`;
- usa l'Inter di `silica_ui` per il testo (`font-family="… Inter"`); di Inter sono
  inclusi solo Regular e SemiBold, quindi pesi più alti escono SemiBold.

Le ombre interne del disegno (sottili, fatte con i filtri) quindi non compaiono.

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

1. La chiave `X-Silica-Icon=<id>` nella voce `.desktop` dell'app. Le icone delle app di
   silica-icons sono **riservate alle app di Silica** (Mustache e le prossime): le app di
   terzi, GNOME comprese, non le prendono mai, nemmeno se fanno la stessa cosa.
2. I luoghi del Dock usano le icone dei file (Download → `folder`, Cestino → `trash-empty` o
   `trash-full`).
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

## Cartelle

La cartella generica di silica-icons (`files/folder-generic`, tipo dinamico `folder-symbol`,
spec in `docs/icon-format.md` di quel repo, "Cartella generica") incide sul pannello un
simbolo di `symbols/` e applica una tinta. Codice: `silica_icons.dart` (parsing, cartelle
speciali, scelta dell'icona) e `shell_icon.dart` (composizione).

- `resolveFolderIcon(path, {symbol, key, tint, env})` → `FolderIcon?`: riconosce la cartella
  speciale con `specialFolderKey` (chiavi di `~/.config/user-dirs.dirs` lette da
  `xdgUserDirs`, più `HOME`, `APPLICATIONS` e `LIBRARY`), prende il simbolo dalla mappa
  `folders` del manifest e la tinta (`#rrggbb` o un'etichetta: `red`, `orange`, `yellow`,
  `green`, `blue`, `purple`, `gray`). `symbol` impone un simbolo qualsiasi di `symbols/`;
  `key` dà la chiave alle cartelle senza percorso fisso (`SHARED`, `NETWORK`). Senza simbolo
  né tinta, o con un simbolo che non c'è, torna la cartella semplice (`folder`).
- `ShellIcon(FolderSource(folderIcon), side: …)` la disegna: livelli, simbolo inciso con il
  peso della fascia di `side` (Light da 128 pt, Regular da 64, Semibold sotto, × √φ sotto 32,
  niente sotto 16), poi la tinta come `ColorFilter.matrix` sulla luminanza del tutto.

### Per Mustache

Mustache non dipende dalla shell. Finché questo codice non passa in `silica_ui`, il gestore dei
file segue le stesse regole del manifest (spec in silica-icons) oppure copia
`resolveFolderIcon` e la parte `_folder` di `ShellIcon`. Per ogni cartella della vista:

1. `resolveFolderIcon(percorso, tint: etichettaOColore)`; per le condivisioni e i mount di
   rete `key: 'SHARED'` o `key: 'NETWORK'`, per un simbolo scelto dall'utente `symbol: nome`;
2. disegnarla con `ShellIcon(FolderSource(icona), side: lato)`: il lato decide il peso del
   simbolo, quindi va passato il lato reale della cella (16, 32, 64, 128…), non una tela
   grande poi ridotta.

Le icone dei file e delle cartelle sono a filo del corpo (keyline documento 648 × 824 e
cartella 824 × 648): accanto alle app hanno lo stesso ingombro.

## Limiti degli SVG

flutter_svg non applica i filtri SVG. Al caricamento la shell:

- toglie le copie dei tracciati che servono solo alle ombre (`<use filter=…>`);
- trasforma ellissi e cerchi sfocati in un gradiente radiale;
- ignora gli altri `filter`;
- usa l'Inter di `silica_ui` per il testo (`font-family="… Inter"`); di Inter sono
  inclusi solo Regular e SemiBold, quindi pesi più alti escono SemiBold.

Le ombre interne del disegno (sottili, fatte con i filtri) quindi non compaiono.

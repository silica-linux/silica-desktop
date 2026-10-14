# App icons in the shell

The icons come from the `silica-linux/silica-icons` repo (CC BY-SA 4.0), which is not
included in this repository: the shell reads them from disk. The format is described in
`docs/icon-format.md` of that repo.

## Where it looks for them

In order, the first one containing `apps/<id>/icon.json` or `files/<id>/icon.json` wins:

1. `$SILICA_ICONS_DIR`: for development, the clone of the repo. `cargo xtask run` sets it by
   itself if `silica-icons` is cloned next to this repository (`~/silica/silica-icons`);
2. `silica/icons` in the XDG data folders: `~/.local/share/silica/icons`, then
   `/usr/local/share/silica/icons` and `/usr/share/silica/icons`.

Code: `shell/lib/system/silica_icons.dart`.

## Which icon an app gets

1. The `X-Silica-Icon=<id>` key in the app's `.desktop` entry. The app icons of
   silica-icons are **reserved for Silica's apps** (Mustache and the next ones): third-party
   apps, GNOME's included, never get them, not even if they do the same thing.
2. The Dock's places use the file icons (Downloads → `folder`, Trash → `trash-empty` or
   `trash-full`).
3. Otherwise the freedesktop theme icon (`icon_lookup.dart`), reduced to the 600 keyline
   on a neutral background with the same shape (`FlatSource`).
4. Without any icon, the tile with the initial (`dock/icons.dart`).

## How it draws it

`silica_ui` (`SilicaIcon`, in silica-ui's `lib/src/icons/`), used by the shell and the apps:

- the visible body is the requested side (48 pt in the Dock, 38 pt in Faro); the 1024 canvas
  overflows by 100/824 per side;
- superellipse mask per corner, n 2.6, radius 0.32 × side
  (`plan/reference/icons.md`); faded bright edge and shadow below the body;
- neutral shadow below the layers with `"shadow": "neutral"`;
- dark theme from the `-dark` files, chosen from the platform brightness;
- clock: hands rotated every second (seconds from 64 pt), every 20 s below that;
  calendar: month in Italian and day, updated at midnight.

## Folders

The generic folder of silica-icons (`files/folder-generic`, dynamic type `folder-symbol`,
spec in that repo's `docs/icon-format.md`, "Generic folder") engraves a symbol from
`symbols/` on the panel and applies a tint. Code in `silica_ui`: `silica_icons.dart` (parsing,
special folders) and `silica_icon.dart` (composition); the shell keeps only the choice of an
app's icon (`shell/lib/system/silica_icons.dart`, `silicaIconFor`).

- `resolveFolderIcon(path, {symbol, key, tint, env})` → `FolderIcon?`: recognizes the special
  folder with `specialFolderKey` (keys of `~/.config/user-dirs.dirs` read by
  `xdgUserDirs`, plus `HOME`, `APPLICATIONS` and `LIBRARY`), takes the symbol from the
  manifest's `folders` map and the tint (`#rrggbb` or a label: `red`, `orange`, `yellow`,
  `green`, `blue`, `purple`, `gray`). `symbol` forces any symbol of `symbols/`;
  `key` gives the key to folders without a fixed path (`SHARED`, `NETWORK`). Without a symbol
  or a tint, or with a symbol that does not exist, the plain folder comes back (`folder`).
- `SilicaIcon(FolderSource(folderIcon), side: …)` draws it: layers, symbol engraved with the
  weight of the `side` band (Light from 128 pt, Regular from 64, Semibold below, × √φ below 32,
  nothing below 16), then the tint as a `ColorFilter.matrix` on the luminance of the whole.

### For Mustache

Mustache uses the same code from `silica_ui` (since 0.7.0), without depending on the shell.
For every folder in the view:

1. `resolveFolderIcon(path, tint: labelOrColor)`; for shares and network mounts
   `key: 'SHARED'` or `key: 'NETWORK'`, for a symbol chosen by the user `symbol: name`;
2. draw it with `SilicaIcon(FolderSource(icon), side: side)`: the side decides the weight of
   the symbol, so the real side of the cell must be passed (16, 32, 64, 128…), not a large
   canvas then scaled down.

For the icons of files, folders, volumes and the Trash the side is the **whole canvas**
(silica-icons 0.2: sheet 684 × 904 at (170, 60), folder 944 × 760 at (40, 132), volumes
944 × 583, Trash 656 × 824 with the base at 924), measured on the Finder; the `min_size`
bands are compared with that side. In the Dock, next to the apps, a place instead uses the
apps' canvas (`SilicaIcon(…, besideApps: true)`, silica_ui ≥ 0.17): side = body, canvas
1024/824 of the side.

## SVG limits

flutter_svg does not apply SVG filters. On loading the shell:

- removes the path copies that only serve the shadows (`<use filter=…>`);
- turns blurred ellipses and circles into a radial gradient;
- ignores the other `filter`s;
- uses `silica_ui`'s Inter for the text (`font-family="… Inter"`); only Regular and SemiBold
  of Inter are included, so higher weights come out SemiBold.

The inner shadows of the drawing (thin, made with filters) therefore do not show.

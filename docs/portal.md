# The portal backend (`silica-portal`)

Apps (third-party ones too, Flatpaks included) ask xdg-desktop-portal for the Open and Save
dialogs; Silica answers with its `org.freedesktop.impl.portal.FileChooser` backend
(`crates/portal`), which opens **Mustache in dialog mode** and returns the chosen URIs.

- Session bus: name `org.freedesktop.impl.portal.desktop.silica`, object
  `/org/freedesktop/portal/desktop`; methods `OpenFile` and `SaveFile`, property `version` 4.
- Options used: `accept_label`, `multiple`, `directory`, `current_name`, `current_folder`.
  Filters (`filters`, `current_filter`) and choices (`choices`) not yet.
- Mustache starts from its `.desktop` entry (`dev.frascella.Mustache.desktop`, installed with
  `cargo xtask install-app ../mustache`) with, in addition, `--dart-arg --dialog=open|save` and
  the other arguments (`--title=`, `--accept=`, `--multiple`, `--directory`, `--name=`,
  `--folder=`). Mustache writes one `file://` URI per line on stdout and exits with 0;
  cancelling exits with 1 without writing anything (portal response 1). Missing program or
  error: response 2.
- Per-user installation: `cargo xtask install-portal` writes the `.portal` file
  (`~/.local/share/xdg-desktop-portal/portals/silica.portal`, `UseIn=Silica`), the
  configuration `~/.config/xdg-desktop-portal/silica-portals.conf` (FileChooser → silica, the
  rest gtk) and the D-Bus service that starts `target/release/silica-portal`.
  `XDG_CURRENT_DESKTOP=Silica` is needed in the session.
- Test: `crates/portal/tests/file_chooser.rs` starts the backend on a private `dbus-daemon` with
  a fake entry in place of Mustache and checks arguments and response.

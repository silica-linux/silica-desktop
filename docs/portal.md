# Il backend dei portali (`silica-portal`)

Le app (anche quelle di terzi, Flatpak comprese) chiedono i dialoghi Apri e Salva a
xdg-desktop-portal; Silica risponde con il suo backend `org.freedesktop.impl.portal.FileChooser`
(`crates/portal`), che apre **Mustache in modalità dialogo** e restituisce gli URI scelti.

- Bus di sessione: nome `org.freedesktop.impl.portal.desktop.silica`, oggetto
  `/org/freedesktop/portal/desktop`; metodi `OpenFile` e `SaveFile`, proprietà `version` 4.
- Opzioni usate: `accept_label`, `multiple`, `directory`, `current_name`, `current_folder`.
  I filtri (`filters`, `current_filter`) e le scelte (`choices`) non ancora.
- Mustache si avvia dalla sua voce `.desktop` (`dev.frascella.Mustache.desktop`, installata con
  `cargo xtask install-app ../mustache`) con in più `--dart-arg --dialog=open|save` e gli altri
  argomenti (`--title=`, `--accept=`, `--multiple`, `--directory`, `--name=`, `--folder=`).
  Mustache scrive un URI `file://` per riga sullo stdout ed esce con 0; annullare esce con 1
  senza scrivere niente (risposta 1 del portale). Programma assente o errore: risposta 2.
- Installazione per l'utente: `cargo xtask install-portal` scrive il file `.portal`
  (`~/.local/share/xdg-desktop-portal/portals/silica.portal`, `UseIn=Silica`), la configurazione
  `~/.config/xdg-desktop-portal/silica-portals.conf` (FileChooser → silica, il resto gtk) e il
  servizio D-Bus che avvia `target/release/silica-portal`. Serve `XDG_CURRENT_DESKTOP=Silica`
  nella sessione.
- Test: `crates/portal/tests/file_chooser.rs` avvia il backend su un `dbus-daemon` privato con
  una voce finta al posto di Mustache e controlla argomenti e risposta.

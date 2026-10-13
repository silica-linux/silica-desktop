# silica-daemon

Servizi di sistema di Silica su D-Bus (`crates/daemon`, binario `silica-daemon`). Oggi le
notifiche; il Centro di controllo (rete, Bluetooth, batteria, luminosità, audio) arriva in
M4.4.

```
silica-daemon [--address INDIRIZZO] [--replace]
```

Si collega al bus di sessione (`DBUS_SESSION_BUS_ADDRESS`, o `--address`) e prende i nomi
`org.freedesktop.Notifications` e `dev.frascella.Silica`. Se un altro server delle notifiche
li ha già (annidato in GNOME), esce con un errore; `--replace` li prende se il proprietario
lo consente. Annidato, Ossido avvia un bus di sessione privato per la shell e le sue app
(M4.2).

## org.freedesktop.Notifications

La [specifica freedesktop](https://specifications.freedesktop.org/notification-spec/) 1.2
su `/org/freedesktop/Notifications`, per le app (`notify-send`, libnotify, portali).

- `Notify`: restituisce l'id (mai 0). Con `replaces_id` di una notifica ancora presente la
  sostituisce e tiene l'id.
- `CloseNotification`: chiude con motivo 3; un id che non c'è più non è un errore.
- `GetCapabilities`: `actions`, `body`, `icon-static`, `persistence`. Niente markup: la
  shell mostra il corpo come testo.
- `GetServerInformation`: `Silica`, `Silica`, versione del crate, `1.2`.
- Segnali `NotificationClosed(id, motivo)` e `ActionInvoked(id, chiave)`.

Hint letti: `urgency` (byte o intero), `category`, `desktop-entry`, `image-path`,
`transient`, `resident` (booleani o interi). Gli altri si ignorano, come i tipi sbagliati.

Scadenza: come nel Centro notifiche del riferimento, una notifica resta nello storico finché
l'utente non la chiude. Scade (motivo 1) solo con `expire_timeout` positivo; -1 e 0 non
scadono. Il tempo del banner lo decide la shell.

Limiti contro i client ostili: titolo 1024 caratteri, corpo 8192, nomi 256, 32 voci di
azioni (una chiave senza etichetta si scarta), storico di 256 notifiche (oltre si chiude
la più vecchia con motivo 4).

## dev.frascella.Silica.Notifications1

Su `/dev/frascella/Silica/Notifications1`, per la shell (banner e Centro notifiche).

- `List() → a(ussssasysssbbti)`: lo storico, dalla più vecchia. Campi: id, app, icona,
  titolo, corpo, azioni (chiave, etichetta, appiattite), urgenza, categoria,
  desktop-entry, immagine, transient, resident, ricezione in ms Unix, `expire_timeout`.
- `Dismiss(id)`: chiusa dall'utente (motivo 2). `Clear()`: tutte.
- `InvokeAction(id, chiave)`: emette `ActionInvoked`; senza `resident` poi chiude con
  motivo 2. Una chiave assente è l'errore `InvalidArgs`.
- Segnali `Added(notifica)` (nuova o sostituita: stesso id) e `Removed(id, motivo)`.

## Prova a mano

```
dbus-daemon --session --fork --print-address    # stampa l'indirizzo
silica-daemon --address unix:path=…
DBUS_SESSION_BUS_ADDRESS=unix:path=… notify-send Ciao "dal daemon"
DBUS_SESSION_BUS_ADDRESS=unix:path=… gdbus call --session -d dev.frascella.Silica \
  -o /dev/frascella/Silica/Notifications1 -m dev.frascella.Silica.Notifications1.List
```

Su Ubuntu il profilo AppArmor di `notify-send` blocca la sua chiamata `GetCapabilities`
verso un server non confinato: `notify-send` mostra allora la notifica senza azioni. Le
azioni si provano con i test (`cargo test -p silica-daemon`, su un `dbus-daemon` privato).

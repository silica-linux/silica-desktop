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
lo consente.

## Bus di sessione privato

`ossido --session-bus` (lo passa `cargo xtask run --shell`) avvia un `dbus-daemon` suo e
`silica-daemon` su quel bus, stampa `DBUS_SESSION_BUS_ADDRESS=…` su stdout e passa
l'indirizzo a tutti i client che lancia (`--spawn`, `--spawn-shell`, `app.launch`).
Annidato in GNOME, i nomi della sessione ospite restano suoi; il bus di sistema è quello
della macchina.

`silica-daemon` si cerca accanto all'eseguibile di Ossido, poi nel `PATH`; se manca, Ossido
avvisa e continua senza notifiche. Il bus muore con Ossido anche se Ossido viene ucciso
(`dbus-daemon` gira sotto una shell che termina quando si chiude lo stdin collegato a
Ossido) e `silica-daemon` esce quando perde il bus.

## Il canale silica/dbus

L'embedder dà a Dart D-Bus con valori JSON (`crates/embedder/src/dbus`, lato Dart
`shell/lib/system/dbus.dart`), su un thread a parte. Codec `JSONMethodCodec`:

- `call {bus?, destination, path, interface?, method, signature?, args?}` → la lista degli
  argomenti della risposta. `bus` è `session` (predefinito) o `system`. Gli argomenti si
  convertono secondo `signature` (`"su"`, `"a{sv}"`…); dentro un variante il tipo si
  deduce dal JSON (interi `i` o `x`, decimali `d`, liste di stringhe `as`, oggetti
  `a{sv}`). Strutture → liste, dizionari → mappe. Un errore D-Bus diventa un
  `PlatformException` con il nome dell'errore come codice; dopo 25 s
  `org.freedesktop.DBus.Error.Timeout`.
- `subscribe {bus?, sender?, path?, interface?, member?}` → un id; i segnali arrivano come
  chiamata `signal {id, sender, path, interface, member, args}` (al massimo 256
  iscrizioni). `unsubscribe {id}` → `true` se c'era.

In Dart: `DBus.instance.call(...)`, `getProperty`, `getAll` e `signals(...)` (uno stream:
l'iscrizione si chiude quando lo si cancella).

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

## dev.frascella.Silica.System1

Su `/dev/frascella/Silica/System1`: lo stato del sistema per il Centro di controllo e la barra
dei menu.

- `State() → a{sv}`: chiavi piatte. `wifi.available`, `wifi.enabled`, `wifi.ssid`,
  `wifi.strength` (0–100), `wifi.wired`; `bluetooth.available`, `bluetooth.powered`;
  `battery.present`, `battery.percent`, `battery.charging`; `brightness.available`,
  `brightness` (0–1); `audio.available`, `audio.volume` (0–1), `audio.muted`, `audio.name` (nome dell'uscita,
  `node.description` di PipeWire); `dnd`.
- `SetWifiEnabled(b)`, `SetBluetoothPowered(b)`, `SetVolume(d)`, `SetMuted(b)`,
  `SetBrightness(d)` (0–1, mai a zero), `SetDoNotDisturb(b)`. Una sorgente assente è
  l'errore `org.freedesktop.DBus.Error.Failed`.
- Segnale `Changed(a{sv})` con lo stato intero a ogni cambiamento.

Sorgenti: NetworkManager (`WirelessEnabled`, connessione principale, intensità del punto
d'accesso), BlueZ (il primo adattatore), UPower (`DisplayDevice`, solo se è una batteria) sul
bus di sistema, riletti a ogni loro segnale; la retroilluminazione in `/sys/class/backlight`
(scrittura con `SetBrightness` di logind, altrimenti nel file) e l'audio dell'uscita
predefinita con `wpctl`, riletti ogni secondo. Se un servizio manca la sorgente è assente
(`*.available = false`) e si riprova ogni 10 s.

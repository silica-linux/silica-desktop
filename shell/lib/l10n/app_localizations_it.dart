// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Italian (`it`).
class AppLocalizationsIt extends AppLocalizations {
  AppLocalizationsIt([String locale = 'it']) : super(locale);

  @override
  String get ccUnavailable => 'Non disponibile';

  @override
  String get ccOff => 'Spento';

  @override
  String get ccOn => 'Acceso';

  @override
  String get ccWired => 'Rete via cavo';

  @override
  String get ccNotConnected => 'Non connesso';

  @override
  String get ccDoNotDisturb => 'Non disturbare';

  @override
  String get ccDoNotDisturbOn => 'Attivo';

  @override
  String get ccBattery => 'Batteria';

  @override
  String ccBatteryLevel(int percent) {
    return '$percent%';
  }

  @override
  String ccBatteryCharging(int percent) {
    return '$percent% · in carica';
  }

  @override
  String get ccDisplay => 'Schermo';

  @override
  String get ccSound => 'Suono';

  @override
  String get mediaUntitled => 'Senza titolo';

  @override
  String get mediaNothingPlaying => 'Nessuna\nriproduzione';

  @override
  String knownNetworks(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Reti conosciute',
      one: 'Rete conosciuta',
    );
    return '$_temp0';
  }

  @override
  String get otherNetworks => 'Altre reti';

  @override
  String get soundOutput => 'Uscita';

  @override
  String get hudAudioOutput => 'Uscita audio';

  @override
  String get stackSortBy => 'Ordina per';

  @override
  String get sortName => 'Nome';

  @override
  String get sortDateAdded => 'Data di aggiunta';

  @override
  String get sortDateModified => 'Data di modifica';

  @override
  String get sortDateCreated => 'Data di creazione';

  @override
  String get sortKind => 'Tipo';

  @override
  String get stackShowAs => 'Mostra come';

  @override
  String get stackFolder => 'Cartella';

  @override
  String get stackStack => 'Pila';

  @override
  String get stackViewContentAs => 'Visualizza contenuto come';

  @override
  String get stackFan => 'Ventaglio';

  @override
  String get stackGrid => 'Griglia';

  @override
  String get stackList => 'Elenco';

  @override
  String get stackAutomatic => 'Automatico';

  @override
  String get options => 'Opzioni';

  @override
  String get showInEnclosingFolder => 'Mostra nella cartella che la contiene';

  @override
  String openNamed(String name) {
    return 'Apri \"$name\"';
  }

  @override
  String get magnificationOff => 'Disattiva l\'ingrandimento';

  @override
  String get magnificationOn => 'Attiva l\'ingrandimento';

  @override
  String get showRecentApps => 'Mostra le app recenti';

  @override
  String get open => 'Apri';

  @override
  String get trash => 'Cestino';

  @override
  String get emptyTrashEllipsis => 'Svuota il Cestino…';

  @override
  String get removeFromDock => 'Rimuovi dal Dock';

  @override
  String get keepInDock => 'Mantieni nel Dock';

  @override
  String get showAllWindows => 'Mostra tutte le finestre';

  @override
  String get quit => 'Esci';

  @override
  String get dockRemove => 'Rimuovi';

  @override
  String stackMoreInFolder(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Altri $count nella cartella',
    );
    return '$_temp0';
  }

  @override
  String get openFolder => 'Apri la cartella';

  @override
  String get trashAlertTitle =>
      'Vuoi eliminare definitivamente gli elementi nel Cestino?';

  @override
  String get trashAlertMessage => 'Non si potrà annullare.';

  @override
  String get emptyTrash => 'Svuota il Cestino';

  @override
  String get cancel => 'Annulla';

  @override
  String get faroPlaceholder => 'Cerca con Faro';

  @override
  String get faroApplication => 'Applicazione';

  @override
  String completionOpen(String rest) {
    return '$rest — Apri';
  }

  @override
  String get launcherPlaceholder => 'Applicazioni';

  @override
  String get catProductivity => 'Produttività e finanza';

  @override
  String get catUtilities => 'Utility';

  @override
  String get catCreativity => 'Creatività';

  @override
  String get catEntertainment => 'Svago';

  @override
  String get catSocial => 'Social';

  @override
  String get catReading => 'Informazione e lettura';

  @override
  String get catOther => 'Altro';

  @override
  String get forceQuitTitle => 'Termina le app';

  @override
  String get forceQuitHelp =>
      'Se un\'app non risponde, sceglila nell\'elenco e fai clic su «Termina».';

  @override
  String get forceQuitShortcut => 'Questo pannello si apre anche con ⌘⌥Esc.';

  @override
  String get forceQuitButton => 'Termina';

  @override
  String get menuFile => 'File';

  @override
  String get menuEdit => 'Modifica';

  @override
  String get menuView => 'Vista';

  @override
  String get menuWindow => 'Finestra';

  @override
  String get menuHelp => 'Aiuto';

  @override
  String get aboutSilica => 'Informazioni su Silica';

  @override
  String get systemSettings => 'Impostazioni di sistema…';

  @override
  String get panorama => 'Panorama';

  @override
  String get showDesktop => 'Mostra la scrivania';

  @override
  String get logOut => 'Esci dalla sessione…';

  @override
  String aboutApp(String app) {
    return 'Informazioni su $app';
  }

  @override
  String quitApp(String app) {
    return 'Esci da $app';
  }

  @override
  String get closeWindow => 'Chiudi finestra';

  @override
  String get fullScreen => 'Schermo intero';

  @override
  String get minimize => 'Riduci';

  @override
  String get appWindows => 'Finestre dell\'app';

  @override
  String get nextSpace => 'Spazio successivo';

  @override
  String get previousSpace => 'Spazio precedente';

  @override
  String get halfScreen => 'Metà dello schermo';

  @override
  String get arrange => 'Disponi';

  @override
  String get notificationsTitle => 'Notifiche';

  @override
  String get noNotifications => 'Nessuna notifica';

  @override
  String get collapse => 'Riduci';

  @override
  String get clear => 'Cancella';

  @override
  String get details => 'Dettagli';

  @override
  String screenshotFileName(String date, String time) {
    return 'Schermata del $date alle $time';
  }

  @override
  String get saveTo => 'Salva in';

  @override
  String get delay => 'Ritardo';

  @override
  String get delayNone => 'Nessuno';

  @override
  String delaySeconds(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count secondi',
      one: '1 secondo',
    );
    return '$_temp0';
  }

  @override
  String get thumbnailAfterCapture => 'Miniatura dopo lo scatto';

  @override
  String get optionsButton => 'Opzioni ⌄';

  @override
  String get capture => 'Acquisisci';

  @override
  String faroCompletionAction(String action) {
    return '— $action';
  }

  @override
  String faroSearchIn(String name) {
    return 'Cerca in $name';
  }

  @override
  String get faroSearchWeb => 'Cerca nel web';

  @override
  String get faroFiles => 'File';

  @override
  String faroToday(String time) {
    return 'oggi, $time';
  }

  @override
  String faroYesterday(String time) {
    return 'ieri, $time';
  }

  @override
  String get faroTabKey => 'Tabulatore';

  @override
  String get faroCopy => 'Copia';

  @override
  String get systemInformation => 'Informazioni di sistema';

  @override
  String get recentItems => 'Elementi recenti';

  @override
  String get recentApplications => 'Applicazioni';

  @override
  String get recentDocuments => 'Documenti';

  @override
  String get recentNone => 'Nessuno';

  @override
  String get recentClear => 'Cancella menu';

  @override
  String get forceQuitMenu => 'Uscita forzata…';

  @override
  String get sleep => 'Standby';

  @override
  String get restart => 'Riavvia…';

  @override
  String get restartNow => 'Riavvia';

  @override
  String get shutDown => 'Spegni…';

  @override
  String get shutDownNow => 'Spegni';

  @override
  String get lockScreen => 'Blocca schermo';

  @override
  String logOutUser(String user) {
    return 'Esci da $user…';
  }

  @override
  String logOutUserNow(String user) {
    return 'Esci da $user';
  }

  @override
  String get ccFocus => 'Focus';

  @override
  String get ccFocusOn => 'Focus attivo';

  @override
  String get ccDarkMode => 'Modalità scura';

  @override
  String get ccScreenCapture => 'Acquisizione schermo';

  @override
  String get focusForAnHour => 'Per un\'ora';

  @override
  String get focusUntilEvening => 'Fino a stasera';
}

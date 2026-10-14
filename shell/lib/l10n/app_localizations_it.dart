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
  String get dockApps => 'App';

  @override
  String get newWindow => 'Nuova finestra';

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

  @override
  String get desktopNewFolder => 'Nuova cartella';

  @override
  String get desktopPaste => 'Incolla elemento';

  @override
  String get desktopGetInfo => 'Informazioni';

  @override
  String get desktopChangeWallpaper => 'Cambia sfondo…';

  @override
  String get desktopEditWidgets => 'Modifica widget…';

  @override
  String get desktopUseStacks => 'Usa pile';

  @override
  String get desktopSortBy => 'Ordina per';

  @override
  String get desktopSortNone => 'Nessuno';

  @override
  String get desktopSortName => 'Nome';

  @override
  String get desktopSortKind => 'Tipo';

  @override
  String get desktopSortModified => 'Data di modifica';

  @override
  String get desktopViewOptions => 'Opzioni vista';

  @override
  String get desktopUntitledFolder => 'cartella senza titolo';

  @override
  String get ccNightLight => 'Luce notturna';

  @override
  String get faroScopeApps => 'Applicazioni';

  @override
  String get faroScopeFiles => 'File';

  @override
  String get faroScopeActions => 'Azioni';

  @override
  String get faroScopeClipboard => 'Appunti';

  @override
  String get faroSearchFiles => 'Cerca nei file';

  @override
  String get faroSearchActions => 'Cerca nelle azioni';

  @override
  String get faroSearchClipboard => 'Cerca negli appunti';

  @override
  String get faroClearHistory => 'Cancella cronologia';

  @override
  String get faroImage => 'Immagine';

  @override
  String get faroSystemAction => 'Sistema';

  @override
  String faroCopied(String time) {
    return 'Copiato $time';
  }

  @override
  String get faroActionScreenshotScreen => 'Istantanea dello schermo';

  @override
  String get faroActionScreenshotSelection => 'Istantanea di una parte';

  @override
  String get faroActionScreenshotToolbar => 'Strumenti istantanea';

  @override
  String get faroActionSystemDarkToggle => 'Cambia aspetto chiaro/scuro';

  @override
  String get faroActionSystemMuteToggle => 'Attiva/disattiva audio';

  @override
  String get faroActionSystemVolumeUp => 'Alza il volume';

  @override
  String get faroActionSystemVolumeDown => 'Abbassa il volume';

  @override
  String get faroActionSystemBrightnessUp => 'Aumenta la luminosità';

  @override
  String get faroActionSystemBrightnessDown => 'Diminuisci la luminosità';

  @override
  String get faroActionPanoramaToggle => 'Panorama';

  @override
  String get faroActionPanoramaApp => 'Finestre dell\'app';

  @override
  String get faroActionDesktopShow => 'Mostra la scrivania';

  @override
  String get faroActionSpaceAdd => 'Nuova scrivania';

  @override
  String get faroActionSpaceNext => 'Scrivania successiva';

  @override
  String get faroActionSpacePrevious => 'Scrivania precedente';

  @override
  String get faroActionLauncherToggle => 'App';

  @override
  String get faroActionNotificationsToggle => 'Centro Notifiche';

  @override
  String get faroActionControlcenterToggle => 'Centro di Controllo';

  @override
  String get faroActionWindowMinimize => 'Riduci la finestra';

  @override
  String get faroActionWindowFullscreen => 'Schermo intero';

  @override
  String get faroActionWindowTileLeft => 'Metà sinistra';

  @override
  String get faroActionWindowTileRight => 'Metà destra';

  @override
  String get faroActionWindowTileTop => 'Metà superiore';

  @override
  String get faroActionWindowTileBottom => 'Metà inferiore';

  @override
  String get faroActionWindowTileFill => 'Riempi';

  @override
  String get faroActionWindowArrange2 => 'Disponi due finestre';

  @override
  String get faroActionWindowArrange3 => 'Disponi tre finestre';

  @override
  String get faroActionWindowArrange4 => 'Disponi quattro finestre';

  @override
  String get faroActionAppForceQuitPanel => 'Uscita forzata';

  @override
  String get faroActionSessionQuit => 'Esci dalla sessione';
}

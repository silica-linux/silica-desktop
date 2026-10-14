// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get ccUnavailable => 'Unavailable';

  @override
  String get ccOff => 'Off';

  @override
  String get ccOn => 'On';

  @override
  String get ccWired => 'Wired network';

  @override
  String get ccNotConnected => 'Not connected';

  @override
  String get ccDoNotDisturb => 'Do not disturb';

  @override
  String get ccDoNotDisturbOn => 'On';

  @override
  String get ccBattery => 'Battery';

  @override
  String ccBatteryLevel(int percent) {
    return '$percent%';
  }

  @override
  String ccBatteryCharging(int percent) {
    return '$percent% · charging';
  }

  @override
  String get ccDisplay => 'Display';

  @override
  String get ccSound => 'Sound';

  @override
  String get mediaUntitled => 'Untitled';

  @override
  String get mediaNothingPlaying => 'Nothing\nplaying';

  @override
  String knownNetworks(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Known networks',
      one: 'Known network',
    );
    return '$_temp0';
  }

  @override
  String get otherNetworks => 'Other networks';

  @override
  String get soundOutput => 'Output';

  @override
  String get hudAudioOutput => 'Audio output';

  @override
  String get stackSortBy => 'Sort by';

  @override
  String get sortName => 'Name';

  @override
  String get sortDateAdded => 'Date added';

  @override
  String get sortDateModified => 'Date modified';

  @override
  String get sortDateCreated => 'Date created';

  @override
  String get sortKind => 'Kind';

  @override
  String get stackShowAs => 'Show as';

  @override
  String get stackFolder => 'Folder';

  @override
  String get stackStack => 'Stack';

  @override
  String get stackViewContentAs => 'View content as';

  @override
  String get stackFan => 'Fan';

  @override
  String get stackGrid => 'Grid';

  @override
  String get stackList => 'List';

  @override
  String get stackAutomatic => 'Automatic';

  @override
  String get options => 'Options';

  @override
  String get showInEnclosingFolder => 'Show in enclosing folder';

  @override
  String openNamed(String name) {
    return 'Open \"$name\"';
  }

  @override
  String get magnificationOff => 'Turn magnification off';

  @override
  String get magnificationOn => 'Turn magnification on';

  @override
  String get showRecentApps => 'Show recent apps';

  @override
  String get open => 'Open';

  @override
  String get trash => 'Trash';

  @override
  String get dockApps => 'Apps';

  @override
  String get newWindow => 'New Window';

  @override
  String get emptyTrashEllipsis => 'Empty Trash…';

  @override
  String get removeFromDock => 'Remove from Dock';

  @override
  String get keepInDock => 'Keep in Dock';

  @override
  String get showAllWindows => 'Show all windows';

  @override
  String get quit => 'Quit';

  @override
  String get dockRemove => 'Remove';

  @override
  String stackMoreInFolder(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count more in the folder',
      one: '1 more in the folder',
    );
    return '$_temp0';
  }

  @override
  String get openFolder => 'Open folder';

  @override
  String get trashAlertTitle => 'Erase the items in the Trash for good?';

  @override
  String get trashAlertMessage => 'This cannot be undone.';

  @override
  String get emptyTrash => 'Empty Trash';

  @override
  String get cancel => 'Cancel';

  @override
  String get faroPlaceholder => 'Search with Faro';

  @override
  String get faroApplication => 'Application';

  @override
  String completionOpen(String rest) {
    return '$rest — Open';
  }

  @override
  String get launcherPlaceholder => 'Applications';

  @override
  String get catProductivity => 'Productivity and finance';

  @override
  String get catUtilities => 'Utilities';

  @override
  String get catCreativity => 'Creativity';

  @override
  String get catEntertainment => 'Entertainment';

  @override
  String get catSocial => 'Social';

  @override
  String get catReading => 'Information and reading';

  @override
  String get catOther => 'Other';

  @override
  String get forceQuitTitle => 'Quit apps';

  @override
  String get forceQuitHelp =>
      'If an app does not respond, choose it in the list and click \"Quit\".';

  @override
  String get forceQuitShortcut => 'This panel also opens with ⌘⌥Esc.';

  @override
  String get forceQuitButton => 'Quit';

  @override
  String get menuFile => 'File';

  @override
  String get menuEdit => 'Edit';

  @override
  String get menuView => 'View';

  @override
  String get menuWindow => 'Window';

  @override
  String get menuHelp => 'Help';

  @override
  String get aboutSilica => 'About Silica';

  @override
  String get systemSettings => 'Settings…';

  @override
  String get panorama => 'Panorama';

  @override
  String get showDesktop => 'Show desktop';

  @override
  String get logOut => 'Log out…';

  @override
  String aboutApp(String app) {
    return 'About $app';
  }

  @override
  String quitApp(String app) {
    return 'Quit $app';
  }

  @override
  String get closeWindow => 'Close window';

  @override
  String get fullScreen => 'Full screen';

  @override
  String get minimize => 'Minimize';

  @override
  String get appWindows => 'App windows';

  @override
  String get nextSpace => 'Next space';

  @override
  String get previousSpace => 'Previous space';

  @override
  String get halfScreen => 'Half of the screen';

  @override
  String get arrange => 'Arrange';

  @override
  String get notificationsTitle => 'Notifications';

  @override
  String get noNotifications => 'No Recent Notifications';

  @override
  String get collapse => 'Collapse';

  @override
  String get clear => 'Clear';

  @override
  String get details => 'Details';

  @override
  String screenshotFileName(String date, String time) {
    return 'Screenshot $date at $time';
  }

  @override
  String get saveTo => 'Save to';

  @override
  String get delay => 'Delay';

  @override
  String get delayNone => 'None';

  @override
  String delaySeconds(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count seconds',
      one: '1 second',
    );
    return '$_temp0';
  }

  @override
  String get thumbnailAfterCapture => 'Thumbnail after capture';

  @override
  String get optionsButton => 'Options ⌄';

  @override
  String get capture => 'Capture';

  @override
  String faroCompletionAction(String action) {
    return '— $action';
  }

  @override
  String faroSearchIn(String name) {
    return 'Search in $name';
  }

  @override
  String get faroSearchWeb => 'Search the Web';

  @override
  String get faroFiles => 'Files';

  @override
  String faroToday(String time) {
    return 'today, $time';
  }

  @override
  String faroYesterday(String time) {
    return 'yesterday, $time';
  }

  @override
  String get faroTabKey => 'Tab';

  @override
  String get faroCopy => 'Copy';

  @override
  String get systemInformation => 'System Information';

  @override
  String get recentItems => 'Recent Items';

  @override
  String get recentApplications => 'Applications';

  @override
  String get recentDocuments => 'Documents';

  @override
  String get recentNone => 'None';

  @override
  String get recentClear => 'Clear Menu';

  @override
  String get forceQuitMenu => 'Force Quit…';

  @override
  String get sleep => 'Sleep';

  @override
  String get restart => 'Restart…';

  @override
  String get restartNow => 'Restart';

  @override
  String get shutDown => 'Shut Down…';

  @override
  String get shutDownNow => 'Shut Down';

  @override
  String get lockScreen => 'Lock Screen';

  @override
  String logOutUser(String user) {
    return 'Log Out $user…';
  }

  @override
  String logOutUserNow(String user) {
    return 'Log Out $user';
  }

  @override
  String get ccFocus => 'Focus';

  @override
  String get ccFocusOn => 'Focus on';

  @override
  String get ccDarkMode => 'Dark Mode';

  @override
  String get ccScreenCapture => 'Screen Capture';

  @override
  String get focusForAnHour => 'For 1 hour';

  @override
  String get focusUntilEvening => 'Until this evening';

  @override
  String get desktopNewFolder => 'New Folder';

  @override
  String get desktopPaste => 'Paste Item';

  @override
  String get desktopGetInfo => 'Get Info';

  @override
  String get desktopChangeWallpaper => 'Change Wallpaper…';

  @override
  String get desktopEditWidgets => 'Edit Widgets…';

  @override
  String get desktopUseStacks => 'Use Stacks';

  @override
  String get desktopSortBy => 'Sort By';

  @override
  String get desktopSortNone => 'None';

  @override
  String get desktopSortName => 'Name';

  @override
  String get desktopSortKind => 'Kind';

  @override
  String get desktopSortModified => 'Date Modified';

  @override
  String get desktopViewOptions => 'Show View Options';

  @override
  String get desktopUntitledFolder => 'untitled folder';

  @override
  String get ccNightLight => 'Night Light';

  @override
  String get chooseWidgets => 'Choose Widgets';

  @override
  String get clearAllNotifications => 'Clear All Notifications';

  @override
  String get replyPlaceholder => 'Reply';

  @override
  String get powerRestartTitle => 'Restart your computer now?';

  @override
  String powerRestartBody(int seconds) {
    return 'Silica will restart automatically in $seconds seconds.';
  }

  @override
  String get powerRestartButton => 'Restart';

  @override
  String get powerShutDownTitle => 'Shut down your computer now?';

  @override
  String powerShutDownBody(int seconds) {
    return 'Silica will shut down automatically in $seconds seconds.';
  }

  @override
  String get powerShutDownButton => 'Shut Down';

  @override
  String powerLogOutTitle(String user) {
    return 'Log out $user and quit all apps?';
  }

  @override
  String powerLogOutBody(int seconds) {
    return 'You will be logged out automatically in $seconds seconds.';
  }

  @override
  String get powerLogOutButton => 'Log Out';

  @override
  String get powerReopenWindows => 'Reopen windows when logging back in';

  @override
  String get batteryMenuTitle => 'Battery';

  @override
  String batteryMenuCharge(int percent) {
    return '$percent % charged';
  }

  @override
  String get batteryMenuOnBattery => 'Power Source: Battery';

  @override
  String get batteryMenuPowerAdapter => 'Power Source: Power Adapter';

  @override
  String get batteryMenuSettings => 'Battery Settings…';

  @override
  String get silicaHelp => 'Silica Help';
}

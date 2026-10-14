import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
import 'app_localizations_it.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations? of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations);
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('en'),
    Locale('it'),
  ];

  /// Control Center module subtitle when the service is missing
  ///
  /// In en, this message translates to:
  /// **'Unavailable'**
  String get ccUnavailable;

  /// Control Center module subtitle: switched off
  ///
  /// In en, this message translates to:
  /// **'Off'**
  String get ccOff;

  /// Bluetooth module subtitle: powered
  ///
  /// In en, this message translates to:
  /// **'On'**
  String get ccOn;

  /// Wi-Fi module subtitle when only a cable is connected
  ///
  /// In en, this message translates to:
  /// **'Wired network'**
  String get ccWired;

  /// Wi-Fi module subtitle: on but not connected
  ///
  /// In en, this message translates to:
  /// **'Not connected'**
  String get ccNotConnected;

  /// Control Center module title
  ///
  /// In en, this message translates to:
  /// **'Do not disturb'**
  String get ccDoNotDisturb;

  /// Do not disturb module subtitle when active
  ///
  /// In en, this message translates to:
  /// **'On'**
  String get ccDoNotDisturbOn;

  /// Control Center module title
  ///
  /// In en, this message translates to:
  /// **'Battery'**
  String get ccBattery;

  /// Battery module subtitle
  ///
  /// In en, this message translates to:
  /// **'{percent}%'**
  String ccBatteryLevel(int percent);

  /// Battery module subtitle while charging
  ///
  /// In en, this message translates to:
  /// **'{percent}% · charging'**
  String ccBatteryCharging(int percent);

  /// Display module and HUD title (brightness)
  ///
  /// In en, this message translates to:
  /// **'Display'**
  String get ccDisplay;

  /// Sound module title
  ///
  /// In en, this message translates to:
  /// **'Sound'**
  String get ccSound;

  /// Media module: track without a title
  ///
  /// In en, this message translates to:
  /// **'Untitled'**
  String get mediaUntitled;

  /// Media module when no player is active (two lines)
  ///
  /// In en, this message translates to:
  /// **'Nothing\nplaying'**
  String get mediaNothingPlaying;

  /// Wi-Fi detail section header
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Known network} other{Known networks}}'**
  String knownNetworks(int count);

  /// Wi-Fi detail: expandable section
  ///
  /// In en, this message translates to:
  /// **'Other networks'**
  String get otherNetworks;

  /// Sound detail section header
  ///
  /// In en, this message translates to:
  /// **'Output'**
  String get soundOutput;

  /// Volume HUD title when the output has no name
  ///
  /// In en, this message translates to:
  /// **'Audio output'**
  String get hudAudioOutput;

  /// Dock stack menu section
  ///
  /// In en, this message translates to:
  /// **'Sort by'**
  String get stackSortBy;

  /// Sort option
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get sortName;

  /// Sort option
  ///
  /// In en, this message translates to:
  /// **'Date added'**
  String get sortDateAdded;

  /// Sort option
  ///
  /// In en, this message translates to:
  /// **'Date modified'**
  String get sortDateModified;

  /// Sort option
  ///
  /// In en, this message translates to:
  /// **'Date created'**
  String get sortDateCreated;

  /// Sort option
  ///
  /// In en, this message translates to:
  /// **'Kind'**
  String get sortKind;

  /// Dock stack menu section
  ///
  /// In en, this message translates to:
  /// **'Show as'**
  String get stackShowAs;

  /// Show the Dock folder as its folder icon
  ///
  /// In en, this message translates to:
  /// **'Folder'**
  String get stackFolder;

  /// Show the Dock folder as a pile of its files
  ///
  /// In en, this message translates to:
  /// **'Stack'**
  String get stackStack;

  /// Dock stack menu section
  ///
  /// In en, this message translates to:
  /// **'View content as'**
  String get stackViewContentAs;

  /// Stack view option
  ///
  /// In en, this message translates to:
  /// **'Fan'**
  String get stackFan;

  /// Stack view option
  ///
  /// In en, this message translates to:
  /// **'Grid'**
  String get stackGrid;

  /// Stack view option
  ///
  /// In en, this message translates to:
  /// **'List'**
  String get stackList;

  /// Stack view option
  ///
  /// In en, this message translates to:
  /// **'Automatic'**
  String get stackAutomatic;

  /// Menu item or section with options
  ///
  /// In en, this message translates to:
  /// **'Options'**
  String get options;

  /// Dock folder menu item
  ///
  /// In en, this message translates to:
  /// **'Show in enclosing folder'**
  String get showInEnclosingFolder;

  /// Dock menu item opening a folder or window
  ///
  /// In en, this message translates to:
  /// **'Open \"{name}\"'**
  String openNamed(String name);

  /// Dock separator menu
  ///
  /// In en, this message translates to:
  /// **'Turn magnification off'**
  String get magnificationOff;

  /// Dock separator menu
  ///
  /// In en, this message translates to:
  /// **'Turn magnification on'**
  String get magnificationOn;

  /// Dock separator menu
  ///
  /// In en, this message translates to:
  /// **'Show recent apps'**
  String get showRecentApps;

  /// Menu item
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get open;

  /// The Trash in the Dock
  ///
  /// In en, this message translates to:
  /// **'Trash'**
  String get trash;

  /// Trash menu item (asks for confirmation)
  ///
  /// In en, this message translates to:
  /// **'Empty Trash…'**
  String get emptyTrashEllipsis;

  /// App options in the Dock
  ///
  /// In en, this message translates to:
  /// **'Remove from Dock'**
  String get removeFromDock;

  /// App options in the Dock
  ///
  /// In en, this message translates to:
  /// **'Keep in Dock'**
  String get keepInDock;

  /// Dock app menu
  ///
  /// In en, this message translates to:
  /// **'Show all windows'**
  String get showAllWindows;

  /// Dock app menu
  ///
  /// In en, this message translates to:
  /// **'Quit'**
  String get quit;

  /// Capsule over an icon dragged out of the Dock
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get dockRemove;

  /// Last item of a fan stack when files are left out
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 more in the folder} other{{count} more in the folder}}'**
  String stackMoreInFolder(int count);

  /// Last item of a stack
  ///
  /// In en, this message translates to:
  /// **'Open folder'**
  String get openFolder;

  /// Empty Trash alert
  ///
  /// In en, this message translates to:
  /// **'Erase the items in the Trash for good?'**
  String get trashAlertTitle;

  /// Empty Trash alert
  ///
  /// In en, this message translates to:
  /// **'This cannot be undone.'**
  String get trashAlertMessage;

  /// Empty Trash alert button
  ///
  /// In en, this message translates to:
  /// **'Empty Trash'**
  String get emptyTrash;

  /// Button
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// Faro search field placeholder
  ///
  /// In en, this message translates to:
  /// **'Search with Faro'**
  String get faroPlaceholder;

  /// Faro result subtitle for apps
  ///
  /// In en, this message translates to:
  /// **'Application'**
  String get faroApplication;

  /// Inline completion in the launcher field
  ///
  /// In en, this message translates to:
  /// **'{rest} — Open'**
  String completionOpen(String rest);

  /// Launcher search field placeholder
  ///
  /// In en, this message translates to:
  /// **'Applications'**
  String get launcherPlaceholder;

  /// Launcher category
  ///
  /// In en, this message translates to:
  /// **'Productivity and finance'**
  String get catProductivity;

  /// Launcher category
  ///
  /// In en, this message translates to:
  /// **'Utilities'**
  String get catUtilities;

  /// Launcher category
  ///
  /// In en, this message translates to:
  /// **'Creativity'**
  String get catCreativity;

  /// Launcher category
  ///
  /// In en, this message translates to:
  /// **'Entertainment'**
  String get catEntertainment;

  /// Launcher category
  ///
  /// In en, this message translates to:
  /// **'Social'**
  String get catSocial;

  /// Launcher category
  ///
  /// In en, this message translates to:
  /// **'Information and reading'**
  String get catReading;

  /// Launcher category for the rest
  ///
  /// In en, this message translates to:
  /// **'Other'**
  String get catOther;

  /// Force quit panel title
  ///
  /// In en, this message translates to:
  /// **'Quit apps'**
  String get forceQuitTitle;

  /// Force quit panel explanation
  ///
  /// In en, this message translates to:
  /// **'If an app does not respond, choose it in the list and click \"Quit\".'**
  String get forceQuitHelp;

  /// Force quit panel hint
  ///
  /// In en, this message translates to:
  /// **'This panel also opens with ⌘⌥Esc.'**
  String get forceQuitShortcut;

  /// Force quit panel button
  ///
  /// In en, this message translates to:
  /// **'Quit'**
  String get forceQuitButton;

  /// Menu bar menu
  ///
  /// In en, this message translates to:
  /// **'File'**
  String get menuFile;

  /// Menu bar menu
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get menuEdit;

  /// Menu bar menu
  ///
  /// In en, this message translates to:
  /// **'View'**
  String get menuView;

  /// Menu bar menu
  ///
  /// In en, this message translates to:
  /// **'Window'**
  String get menuWindow;

  /// Menu bar menu
  ///
  /// In en, this message translates to:
  /// **'Help'**
  String get menuHelp;

  /// Logo menu
  ///
  /// In en, this message translates to:
  /// **'About Silica'**
  String get aboutSilica;

  /// Logo menu
  ///
  /// In en, this message translates to:
  /// **'Settings…'**
  String get systemSettings;

  /// Logo menu: overview of windows and spaces
  ///
  /// In en, this message translates to:
  /// **'Panorama'**
  String get panorama;

  /// Logo menu
  ///
  /// In en, this message translates to:
  /// **'Show desktop'**
  String get showDesktop;

  /// Logo menu
  ///
  /// In en, this message translates to:
  /// **'Log out…'**
  String get logOut;

  /// App menu
  ///
  /// In en, this message translates to:
  /// **'About {app}'**
  String aboutApp(String app);

  /// App menu
  ///
  /// In en, this message translates to:
  /// **'Quit {app}'**
  String quitApp(String app);

  /// File menu
  ///
  /// In en, this message translates to:
  /// **'Close window'**
  String get closeWindow;

  /// View and Window menus, window popover
  ///
  /// In en, this message translates to:
  /// **'Full screen'**
  String get fullScreen;

  /// Window menu
  ///
  /// In en, this message translates to:
  /// **'Minimize'**
  String get minimize;

  /// Window menu
  ///
  /// In en, this message translates to:
  /// **'App windows'**
  String get appWindows;

  /// Window menu
  ///
  /// In en, this message translates to:
  /// **'Next space'**
  String get nextSpace;

  /// Window menu
  ///
  /// In en, this message translates to:
  /// **'Previous space'**
  String get previousSpace;

  /// Window popover section
  ///
  /// In en, this message translates to:
  /// **'Half of the screen'**
  String get halfScreen;

  /// Window popover section
  ///
  /// In en, this message translates to:
  /// **'Arrange'**
  String get arrange;

  /// Notification Center header
  ///
  /// In en, this message translates to:
  /// **'Notifications'**
  String get notificationsTitle;

  /// Notification Center empty state
  ///
  /// In en, this message translates to:
  /// **'No notifications'**
  String get noNotifications;

  /// Notification group pill
  ///
  /// In en, this message translates to:
  /// **'Collapse'**
  String get collapse;

  /// Notification group pill
  ///
  /// In en, this message translates to:
  /// **'Clear'**
  String get clear;

  /// Notification banner button
  ///
  /// In en, this message translates to:
  /// **'Details'**
  String get details;

  /// Screenshot file name without extension; date YYYY-MM-DD, time HH.MM.SS
  ///
  /// In en, this message translates to:
  /// **'Screenshot {date} at {time}'**
  String screenshotFileName(String date, String time);

  /// Screenshot options section
  ///
  /// In en, this message translates to:
  /// **'Save to'**
  String get saveTo;

  /// Screenshot options section
  ///
  /// In en, this message translates to:
  /// **'Delay'**
  String get delay;

  /// Screenshot delay option
  ///
  /// In en, this message translates to:
  /// **'None'**
  String get delayNone;

  /// Screenshot delay option
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 second} other{{count} seconds}}'**
  String delaySeconds(int count);

  /// Screenshot option
  ///
  /// In en, this message translates to:
  /// **'Thumbnail after capture'**
  String get thumbnailAfterCapture;

  /// Screenshot bar button
  ///
  /// In en, this message translates to:
  /// **'Options ⌄'**
  String get optionsButton;

  /// Screenshot bar button
  ///
  /// In en, this message translates to:
  /// **'Capture'**
  String get capture;

  /// Inline completion in the Faro field for a fallback search row
  ///
  /// In en, this message translates to:
  /// **'— {action}'**
  String faroCompletionAction(String action);

  /// Faro fallback row (file manager) and the hint on a chosen folder row
  ///
  /// In en, this message translates to:
  /// **'Search in {name}'**
  String faroSearchIn(String name);

  /// Faro fallback row that searches the web
  ///
  /// In en, this message translates to:
  /// **'Search the Web'**
  String get faroSearchWeb;

  /// Faro: name of the file search when no file manager is installed
  ///
  /// In en, this message translates to:
  /// **'Files'**
  String get faroFiles;

  /// Faro folder row subtitle: modified today
  ///
  /// In en, this message translates to:
  /// **'today, {time}'**
  String faroToday(String time);

  /// Faro folder row subtitle: modified yesterday
  ///
  /// In en, this message translates to:
  /// **'yesterday, {time}'**
  String faroYesterday(String time);

  /// Name of the Tab key in Faro's key capsule
  ///
  /// In en, this message translates to:
  /// **'Tab'**
  String get faroTabKey;

  /// Accessibility label of the copy button in Faro's answer card
  ///
  /// In en, this message translates to:
  /// **'Copy'**
  String get faroCopy;

  /// Logo menu: first item while Option is held
  ///
  /// In en, this message translates to:
  /// **'System Information'**
  String get systemInformation;

  /// Logo menu: submenu of recent apps and documents
  ///
  /// In en, this message translates to:
  /// **'Recent Items'**
  String get recentItems;

  /// Recent Items submenu: section of apps
  ///
  /// In en, this message translates to:
  /// **'Applications'**
  String get recentApplications;

  /// Recent Items submenu: section of documents
  ///
  /// In en, this message translates to:
  /// **'Documents'**
  String get recentDocuments;

  /// Recent Items submenu: an empty section
  ///
  /// In en, this message translates to:
  /// **'None'**
  String get recentNone;

  /// Recent Items submenu: forget the recent items
  ///
  /// In en, this message translates to:
  /// **'Clear Menu'**
  String get recentClear;

  /// Logo menu: opens the force quit panel
  ///
  /// In en, this message translates to:
  /// **'Force Quit…'**
  String get forceQuitMenu;

  /// Logo menu: suspend the computer
  ///
  /// In en, this message translates to:
  /// **'Sleep'**
  String get sleep;

  /// Logo menu: restart with confirmation
  ///
  /// In en, this message translates to:
  /// **'Restart…'**
  String get restart;

  /// Logo menu: restart without confirmation (Option held)
  ///
  /// In en, this message translates to:
  /// **'Restart'**
  String get restartNow;

  /// Logo menu: shut down with confirmation
  ///
  /// In en, this message translates to:
  /// **'Shut Down…'**
  String get shutDown;

  /// Logo menu: shut down without confirmation (Option held)
  ///
  /// In en, this message translates to:
  /// **'Shut Down'**
  String get shutDownNow;

  /// Logo menu: lock the screen
  ///
  /// In en, this message translates to:
  /// **'Lock Screen'**
  String get lockScreen;

  /// Logo menu: log out, with the user's full name
  ///
  /// In en, this message translates to:
  /// **'Log Out {user}…'**
  String logOutUser(String user);

  /// Logo menu: log out without confirmation (Option held)
  ///
  /// In en, this message translates to:
  /// **'Log Out {user}'**
  String logOutUserNow(String user);

  /// Control Center: Focus module and detail title
  ///
  /// In en, this message translates to:
  /// **'Focus'**
  String get ccFocus;

  /// Control Center Focus detail: subtitle while a Focus is on
  ///
  /// In en, this message translates to:
  /// **'Focus on'**
  String get ccFocusOn;

  /// Control Center: accessibility label of the Dark Mode toggle
  ///
  /// In en, this message translates to:
  /// **'Dark Mode'**
  String get ccDarkMode;

  /// Control Center: accessibility label of the Screen Capture toggle
  ///
  /// In en, this message translates to:
  /// **'Screen Capture'**
  String get ccScreenCapture;

  /// Control Center Focus detail: keep Do Not Disturb on for an hour
  ///
  /// In en, this message translates to:
  /// **'For 1 hour'**
  String get focusForAnHour;

  /// Control Center Focus detail: keep Do Not Disturb on until the evening
  ///
  /// In en, this message translates to:
  /// **'Until this evening'**
  String get focusUntilEvening;

  /// Desktop context menu
  ///
  /// In en, this message translates to:
  /// **'New Folder'**
  String get desktopNewFolder;

  /// Desktop context menu
  ///
  /// In en, this message translates to:
  /// **'Paste Item'**
  String get desktopPaste;

  /// Desktop context menu
  ///
  /// In en, this message translates to:
  /// **'Get Info'**
  String get desktopGetInfo;

  /// Desktop context menu: opens the wallpaper settings
  ///
  /// In en, this message translates to:
  /// **'Change Wallpaper…'**
  String get desktopChangeWallpaper;

  /// Desktop context menu
  ///
  /// In en, this message translates to:
  /// **'Edit Widgets…'**
  String get desktopEditWidgets;

  /// Desktop context menu
  ///
  /// In en, this message translates to:
  /// **'Use Stacks'**
  String get desktopUseStacks;

  /// Desktop context menu: submenu
  ///
  /// In en, this message translates to:
  /// **'Sort By'**
  String get desktopSortBy;

  /// Desktop context menu › Sort By
  ///
  /// In en, this message translates to:
  /// **'None'**
  String get desktopSortNone;

  /// Desktop context menu › Sort By
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get desktopSortName;

  /// Desktop context menu › Sort By
  ///
  /// In en, this message translates to:
  /// **'Kind'**
  String get desktopSortKind;

  /// Desktop context menu › Sort By
  ///
  /// In en, this message translates to:
  /// **'Date Modified'**
  String get desktopSortModified;

  /// Desktop context menu
  ///
  /// In en, this message translates to:
  /// **'Show View Options'**
  String get desktopViewOptions;

  /// Name of a folder created from the desktop context menu
  ///
  /// In en, this message translates to:
  /// **'untitled folder'**
  String get desktopUntitledFolder;

  /// Control Center Display detail: warmer colors at night
  ///
  /// In en, this message translates to:
  /// **'Night Light'**
  String get ccNightLight;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en', 'it'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
    case 'it':
      return AppLocalizationsIt();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}

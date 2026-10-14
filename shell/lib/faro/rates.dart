// Exchange rates for Faro's currency conversions (SH-SP-14): the European Central Bank's
// daily reference rates (euro-based, ~30 currencies, updated on working days around 16:00
// CET). They are fetched when Faro opens and kept in `~/.cache/silica/eurofxref.xml`, used
// until 12 hours old; offline, the last copy is used whatever its age.

import 'dart:async';
import 'dart:io';

/// The ECB's daily rates.
const String ratesUrl =
    'https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml';

/// How long a fetched copy is fresh.
const Duration ratesMaxAge = Duration(hours: 12);

/// Rates per euro, by ISO code (the euro itself is 1), and their date.
class CurrencyRates {
  CurrencyRates({File? cache})
    : _cache = cache ?? File('${_cacheDir()}/silica/eurofxref.xml');

  final File _cache;
  Map<String, double> _rates = const {};
  DateTime? _date;
  bool _fetching = false;

  Map<String, double> get rates => _rates;

  /// The day of the rates (the ECB's date), null before they are loaded.
  DateTime? get date => _date;

  static String _cacheDir() {
    final env = Platform.environment;
    return env['XDG_CACHE_HOME'] ??
        '${env['HOME'] ?? Directory.systemTemp.path}/.cache';
  }

  /// Reads the cached copy, and fetches a new one if it is missing or old.
  Future<void> load({void Function()? onDone}) async {
    if (_rates.isEmpty && _cache.existsSync()) {
      _use(_cache.readAsStringSync());
      onDone?.call();
    }
    final age = _cache.existsSync()
        ? DateTime.now().difference(_cache.lastModifiedSync())
        : null;
    if (_fetching || (age != null && age < ratesMaxAge)) return;
    _fetching = true;
    try {
      final client = HttpClient()
        ..connectionTimeout = const Duration(seconds: 5);
      final request = await client.getUrl(Uri.parse(ratesUrl));
      final response = await request.close().timeout(
        const Duration(seconds: 10),
      );
      if (response.statusCode == 200) {
        final text = await response
            .transform(const SystemEncoding().decoder)
            .join()
            .timeout(const Duration(seconds: 10));
        if (_use(text)) {
          _cache.parent.createSync(recursive: true);
          _cache.writeAsStringSync(text);
          onDone?.call();
        }
      }
      client.close();
    } on Object {
      // Offline: the cached copy, if any.
    } finally {
      _fetching = false;
    }
  }

  bool _use(String xml) {
    final parsed = parseEcbRates(xml);
    if (parsed == null) return false;
    _rates = parsed.$1;
    _date = parsed.$2;
    return true;
  }

  /// Sets the rates (tests).
  void debugSet(Map<String, double> rates) => _rates = rates;
}

/// The rates in the ECB's XML (`<Cube currency='USD' rate='1.08'/>`), the euro added, and
/// the date (`<Cube time='2026-10-14'>`); null if there are none.
(Map<String, double>, DateTime?)? parseEcbRates(String xml) {
  final rates = <String, double>{'EUR': 1};
  final cube = RegExp(
    r'''currency=['"]([A-Z]{3})['"]\s+rate=['"]([0-9.]+)['"]''',
  );
  for (final m in cube.allMatches(xml)) {
    final rate = double.tryParse(m[2]!);
    if (rate != null && rate > 0) rates[m[1]!] = rate;
  }
  if (rates.length < 2) return null;
  final time = RegExp(r'''time=['"](\d{4}-\d{2}-\d{2})['"]''').firstMatch(xml);
  return (rates, time == null ? null : DateTime.tryParse(time[1]!));
}

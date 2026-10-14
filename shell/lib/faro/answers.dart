// Faro's instant answers: calculations ("12*3" → 36) and unit conversions ("10 km in miles"
// → 6.21 miles), shown as a card under the field and completed inline ("= 36").
// spotlight.md › Inventory INV-A3 (SH-SP-13, SH-SP-14). Currency conversions need exchange
// rates from the network: not done.

import 'dart:math' as math;

/// An answer for the query: the expression as typed and the result, both ready to show.
class FaroAnswer {
  const FaroAnswer({required this.expression, required this.result});

  /// "12*3 =" in the card's first line.
  final String expression;

  /// "36", "6.21 miles".
  final String result;
}

/// The answer for `query`, or null: a conversion first, then a calculation. `decimal` is the
/// decimal separator of the shell's language.
FaroAnswer? answerFor(String query, {String decimal = '.'}) {
  final q = query.trim();
  if (q.isEmpty) return null;
  final conversion = convertUnits(q);
  if (conversion != null) {
    return FaroAnswer(
      expression: '$q =',
      result:
          '${formatNumber(conversion.$1, decimal: decimal)} ${conversion.$2}',
    );
  }
  final value = calculate(q);
  if (value == null) return null;
  return FaroAnswer(
    expression: '$q =',
    result: formatNumber(value, decimal: decimal),
  );
}

/// The number with at most 10 significant digits, without trailing zeros; ±∞ and NaN as
/// they are.
String formatNumber(double value, {String decimal = '.'}) {
  if (value.isNaN || value.isInfinite) return value.toString();
  if (value == value.roundToDouble() && value.abs() < 1e15) {
    return value.toInt().toString();
  }
  var text = value.toStringAsPrecision(10);
  if (text.contains('e')) return text.replaceAll('.', decimal);
  if (text.contains('.')) {
    text = text
        .replaceFirst(RegExp(r'0+$'), '')
        .replaceFirst(RegExp(r'\.$'), '');
  }
  return text.replaceAll('.', decimal);
}

// ---------------------------------------------------------------------------------------
// Calculations

/// The value of an arithmetic expression, or null if `text` is not one (a bare number is not
/// a calculation). Operators + − × ÷ * / ^ %, parentheses, unary minus, decimals with a
/// point or a comma, the constants pi and e, and sqrt, sin, cos, tan, ln, log, abs.
double? calculate(String text) {
  final tokens = _tokenize(text);
  if (tokens == null || tokens.isEmpty) return null;
  // At least an operator or a function: "12", "e" or "pi" alone are searches.
  if (!tokens.any(
    (t) =>
        t.kind == _Kind.op ||
        (t.kind == _Kind.name && _functions.containsKey(t.text)),
  )) {
    return null;
  }
  final parser = _Parser(tokens);
  final value = parser.expression();
  if (value == null || !parser.done || value.isNaN) return null;
  return value;
}

enum _Kind { number, op, open, close, name }

class _Token {
  const _Token(this.kind, [this.text = '', this.value = 0]);
  final _Kind kind;
  final String text;
  final double value;
}

List<_Token>? _tokenize(String text) {
  final tokens = <_Token>[];
  var i = 0;
  while (i < text.length) {
    final c = text[i];
    if (c == ' ') {
      i++;
      continue;
    }
    final number = RegExp(r'\d+(?:[.,]\d+)?').matchAsPrefix(text, i);
    if (number != null) {
      final value = double.tryParse(number[0]!.replaceAll(',', '.'));
      if (value == null) return null;
      tokens.add(_Token(_Kind.number, number[0]!, value));
      i = number.end;
      continue;
    }
    final name = RegExp(r'[a-zA-Zπ]+').matchAsPrefix(text, i);
    if (name != null) {
      final word = name[0]!.toLowerCase();
      if (!_functions.containsKey(word) && !_constants.containsKey(word)) {
        return null;
      }
      tokens.add(_Token(_Kind.name, word));
      i = name.end;
      continue;
    }
    switch (c) {
      case '+' || '-' || '−' || '*' || '×' || '/' || '÷' || '^' || '%':
        tokens.add(_Token(_Kind.op, c));
      case '(':
        tokens.add(const _Token(_Kind.open));
      case ')':
        tokens.add(const _Token(_Kind.close));
      default:
        return null;
    }
    i++;
  }
  return tokens;
}

const Map<String, double> _constants = {
  'pi': math.pi,
  'π': math.pi,
  'e': math.e,
};

final Map<String, double Function(double)> _functions = {
  'sqrt': math.sqrt,
  'sin': math.sin,
  'cos': math.cos,
  'tan': math.tan,
  'ln': math.log,
  'log': (x) => math.log(x) / math.ln10,
  'abs': (x) => x.abs(),
};

/// Recursive descent: sum := product (± product)*, product := power (×÷% power)*,
/// power := unary (^ power)?, unary := −unary | atom.
class _Parser {
  _Parser(this.tokens);
  final List<_Token> tokens;
  int _at = 0;

  bool get done => _at == tokens.length;
  _Token? get _peek => _at < tokens.length ? tokens[_at] : null;

  bool _op(String chars) {
    final t = _peek;
    if (t != null && t.kind == _Kind.op && chars.contains(t.text)) {
      _at++;
      return true;
    }
    return false;
  }

  double? expression() {
    var left = _product();
    while (left != null) {
      if (_op('+')) {
        final right = _product();
        left = right == null ? null : left + right;
      } else if (_op('-−')) {
        final right = _product();
        left = right == null ? null : left - right;
      } else {
        break;
      }
    }
    return left;
  }

  double? _product() {
    var left = _power();
    while (left != null) {
      if (_op('*×')) {
        final right = _power();
        left = right == null ? null : left * right;
      } else if (_op('/÷')) {
        final right = _power();
        left = right == null || right == 0 ? null : left / right;
      } else if (_op('%')) {
        // "50%" is a percentage; "7 % 3" a remainder.
        final next = _peek;
        if (next == null || next.kind == _Kind.close || next.kind == _Kind.op) {
          left = left / 100;
        } else {
          final right = _power();
          left = right == null || right == 0 ? null : left % right;
        }
      } else {
        break;
      }
    }
    return left;
  }

  double? _power() {
    final base = _unary();
    if (base == null) return null;
    if (_op('^')) {
      final exponent = _power();
      return exponent == null ? null : math.pow(base, exponent).toDouble();
    }
    return base;
  }

  double? _unary() {
    if (_op('-−')) {
      final value = _unary();
      return value == null ? null : -value;
    }
    if (_op('+')) return _unary();
    return _atom();
  }

  double? _atom() {
    final t = _peek;
    if (t == null) return null;
    _at++;
    switch (t.kind) {
      case _Kind.number:
        return t.value;
      case _Kind.open:
        final value = expression();
        if (_peek?.kind != _Kind.close) return null;
        _at++;
        return value;
      case _Kind.name:
        if (_constants[t.text] case final c?) return c;
        final f = _functions[t.text];
        final argument = _atom();
        if (f == null || argument == null) return null;
        return f(argument);
      case _Kind.op || _Kind.close:
        return null;
    }
  }
}

// ---------------------------------------------------------------------------------------
// Unit conversions

enum _Quantity { length, mass, volume, temperature, time, speed, data, area }

class _Unit {
  const _Unit(this.quantity, this.factor, [this.offset = 0]);
  final _Quantity quantity;

  /// Value in the base unit = value × factor + offset.
  final double factor;
  final double offset;
}

/// Unit names (English and Italian, singular and plural, symbols), all lowercase.
const Map<String, _Unit> _units = {
  // Length, base metre.
  'mm': _Unit(_Quantity.length, 0.001),
  'millimeter': _Unit(_Quantity.length, 0.001),
  'millimeters': _Unit(_Quantity.length, 0.001),
  'millimetro': _Unit(_Quantity.length, 0.001),
  'millimetri': _Unit(_Quantity.length, 0.001),
  'cm': _Unit(_Quantity.length, 0.01),
  'centimeter': _Unit(_Quantity.length, 0.01),
  'centimeters': _Unit(_Quantity.length, 0.01),
  'centimetro': _Unit(_Quantity.length, 0.01),
  'centimetri': _Unit(_Quantity.length, 0.01),
  'm': _Unit(_Quantity.length, 1),
  'meter': _Unit(_Quantity.length, 1),
  'meters': _Unit(_Quantity.length, 1),
  'metre': _Unit(_Quantity.length, 1),
  'metres': _Unit(_Quantity.length, 1),
  'metro': _Unit(_Quantity.length, 1),
  'metri': _Unit(_Quantity.length, 1),
  'km': _Unit(_Quantity.length, 1000),
  'kilometer': _Unit(_Quantity.length, 1000),
  'kilometers': _Unit(_Quantity.length, 1000),
  'chilometro': _Unit(_Quantity.length, 1000),
  'chilometri': _Unit(_Quantity.length, 1000),
  'in': _Unit(_Quantity.length, 0.0254),
  'inch': _Unit(_Quantity.length, 0.0254),
  'inches': _Unit(_Quantity.length, 0.0254),
  'pollice': _Unit(_Quantity.length, 0.0254),
  'pollici': _Unit(_Quantity.length, 0.0254),
  'ft': _Unit(_Quantity.length, 0.3048),
  'foot': _Unit(_Quantity.length, 0.3048),
  'feet': _Unit(_Quantity.length, 0.3048),
  'piede': _Unit(_Quantity.length, 0.3048),
  'piedi': _Unit(_Quantity.length, 0.3048),
  'yd': _Unit(_Quantity.length, 0.9144),
  'yard': _Unit(_Quantity.length, 0.9144),
  'yards': _Unit(_Quantity.length, 0.9144),
  'mi': _Unit(_Quantity.length, 1609.344),
  'mile': _Unit(_Quantity.length, 1609.344),
  'miles': _Unit(_Quantity.length, 1609.344),
  'miglio': _Unit(_Quantity.length, 1609.344),
  'miglia': _Unit(_Quantity.length, 1609.344),
  // Mass, base kilogram.
  'mg': _Unit(_Quantity.mass, 1e-6),
  'g': _Unit(_Quantity.mass, 0.001),
  'gram': _Unit(_Quantity.mass, 0.001),
  'grams': _Unit(_Quantity.mass, 0.001),
  'grammo': _Unit(_Quantity.mass, 0.001),
  'grammi': _Unit(_Quantity.mass, 0.001),
  'kg': _Unit(_Quantity.mass, 1),
  'kilogram': _Unit(_Quantity.mass, 1),
  'kilograms': _Unit(_Quantity.mass, 1),
  'chilo': _Unit(_Quantity.mass, 1),
  'chili': _Unit(_Quantity.mass, 1),
  'chilogrammo': _Unit(_Quantity.mass, 1),
  'chilogrammi': _Unit(_Quantity.mass, 1),
  't': _Unit(_Quantity.mass, 1000),
  'tonne': _Unit(_Quantity.mass, 1000),
  'tonnes': _Unit(_Quantity.mass, 1000),
  'tonnellata': _Unit(_Quantity.mass, 1000),
  'tonnellate': _Unit(_Quantity.mass, 1000),
  'oz': _Unit(_Quantity.mass, 0.028349523125),
  'ounce': _Unit(_Quantity.mass, 0.028349523125),
  'ounces': _Unit(_Quantity.mass, 0.028349523125),
  'oncia': _Unit(_Quantity.mass, 0.028349523125),
  'once': _Unit(_Quantity.mass, 0.028349523125),
  'lb': _Unit(_Quantity.mass, 0.45359237),
  'lbs': _Unit(_Quantity.mass, 0.45359237),
  'pound': _Unit(_Quantity.mass, 0.45359237),
  'pounds': _Unit(_Quantity.mass, 0.45359237),
  'libbra': _Unit(_Quantity.mass, 0.45359237),
  'libbre': _Unit(_Quantity.mass, 0.45359237),
  // Volume, base litre.
  'ml': _Unit(_Quantity.volume, 0.001),
  'milliliter': _Unit(_Quantity.volume, 0.001),
  'milliliters': _Unit(_Quantity.volume, 0.001),
  'millilitro': _Unit(_Quantity.volume, 0.001),
  'millilitri': _Unit(_Quantity.volume, 0.001),
  'l': _Unit(_Quantity.volume, 1),
  'liter': _Unit(_Quantity.volume, 1),
  'liters': _Unit(_Quantity.volume, 1),
  'litre': _Unit(_Quantity.volume, 1),
  'litres': _Unit(_Quantity.volume, 1),
  'litro': _Unit(_Quantity.volume, 1),
  'litri': _Unit(_Quantity.volume, 1),
  'gal': _Unit(_Quantity.volume, 3.785411784),
  'gallon': _Unit(_Quantity.volume, 3.785411784),
  'gallons': _Unit(_Quantity.volume, 3.785411784),
  'gallone': _Unit(_Quantity.volume, 3.785411784),
  'galloni': _Unit(_Quantity.volume, 3.785411784),
  'cup': _Unit(_Quantity.volume, 0.2365882365),
  'cups': _Unit(_Quantity.volume, 0.2365882365),
  'tazza': _Unit(_Quantity.volume, 0.2365882365),
  'tazze': _Unit(_Quantity.volume, 0.2365882365),
  // Temperature, base kelvin.
  '°c': _Unit(_Quantity.temperature, 1, 273.15),
  'c': _Unit(_Quantity.temperature, 1, 273.15),
  'celsius': _Unit(_Quantity.temperature, 1, 273.15),
  '°f': _Unit(_Quantity.temperature, 5 / 9, 273.15 - 32 * 5 / 9),
  'f': _Unit(_Quantity.temperature, 5 / 9, 273.15 - 32 * 5 / 9),
  'fahrenheit': _Unit(_Quantity.temperature, 5 / 9, 273.15 - 32 * 5 / 9),
  'k': _Unit(_Quantity.temperature, 1),
  'kelvin': _Unit(_Quantity.temperature, 1),
  // Time, base second.
  's': _Unit(_Quantity.time, 1),
  'sec': _Unit(_Quantity.time, 1),
  'second': _Unit(_Quantity.time, 1),
  'seconds': _Unit(_Quantity.time, 1),
  'secondo': _Unit(_Quantity.time, 1),
  'secondi': _Unit(_Quantity.time, 1),
  'min': _Unit(_Quantity.time, 60),
  'minute': _Unit(_Quantity.time, 60),
  'minutes': _Unit(_Quantity.time, 60),
  'minuto': _Unit(_Quantity.time, 60),
  'minuti': _Unit(_Quantity.time, 60),
  'h': _Unit(_Quantity.time, 3600),
  'hour': _Unit(_Quantity.time, 3600),
  'hours': _Unit(_Quantity.time, 3600),
  'ora': _Unit(_Quantity.time, 3600),
  'ore': _Unit(_Quantity.time, 3600),
  'day': _Unit(_Quantity.time, 86400),
  'days': _Unit(_Quantity.time, 86400),
  'giorno': _Unit(_Quantity.time, 86400),
  'giorni': _Unit(_Quantity.time, 86400),
  'week': _Unit(_Quantity.time, 604800),
  'weeks': _Unit(_Quantity.time, 604800),
  'settimana': _Unit(_Quantity.time, 604800),
  'settimane': _Unit(_Quantity.time, 604800),
  // Speed, base metre per second.
  'm/s': _Unit(_Quantity.speed, 1),
  'km/h': _Unit(_Quantity.speed, 1 / 3.6),
  'kmh': _Unit(_Quantity.speed, 1 / 3.6),
  'mph': _Unit(_Quantity.speed, 0.44704),
  'knot': _Unit(_Quantity.speed, 0.514444),
  'knots': _Unit(_Quantity.speed, 0.514444),
  'nodo': _Unit(_Quantity.speed, 0.514444),
  'nodi': _Unit(_Quantity.speed, 0.514444),
  // Data, base byte (decimal multiples, as file managers show them).
  'b': _Unit(_Quantity.data, 1),
  'byte': _Unit(_Quantity.data, 1),
  'bytes': _Unit(_Quantity.data, 1),
  'kb': _Unit(_Quantity.data, 1e3),
  'mb': _Unit(_Quantity.data, 1e6),
  'gb': _Unit(_Quantity.data, 1e9),
  'tb': _Unit(_Quantity.data, 1e12),
  'kib': _Unit(_Quantity.data, 1024),
  'mib': _Unit(_Quantity.data, 1048576),
  'gib': _Unit(_Quantity.data, 1073741824),
  // Area, base square metre.
  'm2': _Unit(_Quantity.area, 1),
  'm²': _Unit(_Quantity.area, 1),
  'km2': _Unit(_Quantity.area, 1e6),
  'km²': _Unit(_Quantity.area, 1e6),
  'ha': _Unit(_Quantity.area, 1e4),
  'hectare': _Unit(_Quantity.area, 1e4),
  'hectares': _Unit(_Quantity.area, 1e4),
  'ettaro': _Unit(_Quantity.area, 1e4),
  'ettari': _Unit(_Quantity.area, 1e4),
  'acre': _Unit(_Quantity.area, 4046.8564224),
  'acres': _Unit(_Quantity.area, 4046.8564224),
  'acro': _Unit(_Quantity.area, 4046.8564224),
  'acri': _Unit(_Quantity.area, 4046.8564224),
};

final RegExp _conversion = RegExp(
  r'^(-?\d+(?:[.,]\d+)?)\s*([^\s\d][^\s]*(?:\s[^\s]+)?)\s+(?:in|to|into|a|→|->)\s+(.+)$',
  caseSensitive: false,
);

/// "10 km in miles" → (6.2137…, "miles"): the value and the target unit as typed. Null if
/// `text` is not a conversion between two units of the same quantity.
(double, String)? convertUnits(String text) {
  final match = _conversion.firstMatch(text.trim());
  if (match == null) return null;
  final value = double.tryParse(match[1]!.replaceAll(',', '.'));
  final from = _units[match[2]!.trim().toLowerCase()];
  final targetText = match[3]!.trim();
  final to = _units[targetText.toLowerCase()];
  if (value == null || from == null || to == null) return null;
  if (from.quantity != to.quantity) return null;
  final base = value * from.factor + from.offset;
  return ((base - to.offset) / to.factor, targetText);
}

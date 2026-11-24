import 'dart:async';
import 'dart:convert';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:flutter/painting.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:path_provider/path_provider.dart';

import 'model.dart';
import 'src/rust/api.dart' as rust;

/// Live state from the paired PC. Until a phone is paired, or before the first status
/// arrives, the screens fall back to demo data (and say so).
class Hub extends ChangeNotifier {
  Hub(this.statePath, this.stateKey);

  /// Pairing file in the app's private storage, encrypted with [stateKey].
  final String statePath;

  /// 32 random bytes kept in the platform keystore (Android Keystore, iOS Keychain,
  /// libsecret on Linux), so a copy of the app's files alone can't impersonate the phone.
  final Uint8List stateKey;

  static Future<Hub> open() async {
    final dir = await getApplicationSupportDirectory();
    const store = FlutterSecureStorage();
    var key = await store.read(key: 'pairing_key');
    if (key == null) {
      final r = Random.secure();
      key = base64Encode(List.generate(32, (_) => r.nextInt(256)));
      await store.write(key: 'pairing_key', value: key);
    }
    final hub = Hub('${dir.path}/pairing.bin', base64Decode(key));
    if (hub.paired) hub.start();
    return hub;
  }

  bool get paired => rust.isPaired(statePath: statePath, stateKey: stateKey);

  List<Child>? children; // null = no status yet
  final Map<String, rust.Rules> rules = {}; // by Linux username
  final Map<String, rust.ChildHistory> history = {};

  /// A message from the PC for the parent (e.g. a refused rule change). Shown once.
  String? notice;
  final List<TimeRequest> requests = [];
  String? problem;
  StreamSubscription<rust.Msg>? _sub;

  bool get live => children != null;

  void start() {
    _sub?.cancel();
    _sub = rust
        .listen(statePath: statePath, stateKey: stateKey)
        .listen(
          _on,
          onError: (Object e) {
            problem = '$e';
            notifyListeners();
          },
        );
    // Ask for everything right away (the PC only sends on changes otherwise).
    for (final ask in [rust.requestStatus, rust.requestRules, rust.requestHistory]) {
      ask(statePath: statePath, stateKey: stateKey).catchError((Object e) {
        problem = '$e';
        notifyListeners();
      });
    }
  }

  /// Redraw after something outside the hub changed (e.g. push became available).
  void touch() => notifyListeners();

  /// A message that arrived some other way (push) goes through the same handling.
  void deliver(rust.Msg m) => _on(m);

  Future<rust.Msg?> openPush(List<int> content) =>
      rust.openPush(statePath: statePath, stateKey: stateKey, payload: Uint8List.fromList(content));

  /// Tell the PC where to send wake-ups (null = stop).
  Future<void> setPush(String? endpoint) async {
    if (!paired) return;
    await rust.setPush(statePath: statePath, stateKey: stateKey, endpoint: endpoint);
  }

  void _on(rust.Msg m) {
    switch (m) {
      case rust.Msg_Status(:final children):
        this.children = [for (final (i, k) in children.indexed) _child(i, k)];
        problem = null;
      case rust.Msg_Request(:final id, :final user, :final minutes, :final reason):
        problem = null;
        final kid = children?.where((c) => c.name == _display(user)).firstOrNull ?? _child(0, null, name: user);
        requests.removeWhere((r) => r.id == id);
        requests.insert(
          0,
          TimeRequest(id: id, child: kid, minutes: minutes, reason: reason, lockedAt: '', bedtime: ''),
        );
      case rust.Msg_History(:final children):
        problem = null;
        for (final h in children) {
          history[h.user] = h;
        }
      case rust.Msg_Rules(:final children):
        problem = null;
        for (final r in children) {
          rules[r.user] = r;
        }
      case rust.Msg_Notice(:final text):
        notice = text;
      case rust.Msg_Online():
        problem = null;
      case rust.Msg_Problem(:final text):
        problem = text;
    }
    notifyListeners();
  }

  /// ponytail: fixed name; a "name this phone" field when there are settings.
  Future<void> pair(String code) async {
    await rust.pair(statePath: statePath, stateKey: stateKey, qr: code, phoneName: "Parent's phone");
    start();
    notifyListeners();
  }

  Future<void> refreshHistory() => rust.requestHistory(statePath: statePath, stateKey: stateKey);

  /// Send a child's new rules. The PC answers with the rules in force (or a notice).
  Future<void> saveRules(rust.Rules r) async {
    rules[r.user] = r;
    notifyListeners();
    await rust.setRules(statePath: statePath, stateKey: stateKey, rules: r);
  }

  /// Colour set for a child, stable across screens: by position in the PC's list.
  (Color, Color, Color) paletteFor(String user) {
    final i = rules.keys.toList().indexOf(user);
    return palette[(i < 0 ? 0 : i) % palette.length];
  }

  /// Answer a request; removes it from the list once sent.
  Future<void> answer(TimeRequest r, {required bool approve, required int minutes}) async {
    await rust.answer(statePath: statePath, stateKey: stateKey, id: r.id, approve: approve, minutes: minutes);
    requests.removeWhere((x) => x.id == r.id);
    notifyListeners();
  }

  @override
  void dispose() {
    _sub?.cancel();
    super.dispose();
  }

  static Child _child(int i, rust.Kid? k, {String? name}) {
    final p = palette[i % palette.length];
    final used = (k?.usedS.toInt() ?? 0) ~/ 60;
    final leftS = k?.leftS.toInt() ?? -1;
    final limit = leftS < 0 ? 0 : used + (leftS + 59) ~/ 60;
    final over = k?.over ?? false;
    return Child(
      name: _display(k?.user ?? name ?? '?'),
      color: p.$1,
      tint: p.$2,
      ink: p.$3,
      usedMin: used,
      limitMin: limit,
      locked: over,
      note: limit == 0 ? '${hm(used)} used · no limit' : '${hm(used)} of ${hm(limit)} used',
      pill: over ? 'Limit reached' : (limit == 0 ? 'No limit' : '${hm(limit - used)} left'),
    );
  }

  /// Linux usernames are lower case ("mia"); show "Mia".
  /// ponytail: a display name per child comes with editable rules.
  static String display(String user) => _display(user);
  static String _display(String user) => user.isEmpty ? user : user[0].toUpperCase() + user.substring(1);

  /// Signature colours per child, in order: (colour, tint, ink on tint).
  static const palette = [
    (Color(0xFFE8622C), Color(0xFFFBE9E0), Color(0xFFA33F14)),
    (Color(0xFF1F9A8A), Color(0xFFE2F3F0), Color(0xFF0F5F55)),
    (Color(0xFF5B6BE0), Color(0xFFE7E9FB), Color(0xFF3442A8)),
    (Color(0xFFC2457A), Color(0xFFF8E4EC), Color(0xFF8E2A55)),
  ];
}

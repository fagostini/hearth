import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:flutter/painting.dart';
import 'package:flutter/widgets.dart' show AppLifecycleListener, AppLifecycleState, WidgetsBinding;
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
  /// Empty until the first pairing creates it (see [pair]).
  Uint8List stateKey;

  static const _store = FlutterSecureStorage(aOptions: AndroidOptions(resetOnError: false));

  static Future<Hub> open() async {
    final dir = await getApplicationSupportDirectory();
    // resetOnError (default true) would silently wipe the key on one failed keystore
    // read, unpairing the phone. Throw instead; retry once.
    const store = _store;
    String? key;
    try {
      key = await store.read(key: 'pairing_key');
    } catch (e) {
      debugPrint('hearth: key read failed ($e), retrying');
      await Future<void>.delayed(const Duration(milliseconds: 500));
      key = await store.read(key: 'pairing_key');
    }
    if (key == null) {
      // No key: any pairing file is unreadable, so start clean rather than half-paired.
      final old = File('${dir.path}/pairing.bin');
      if (old.existsSync()) {
        debugPrint('hearth: pairing key missing, removing unreadable pairing');
        old.deleteSync();
      }
    }
    // No key is made here: a push can start a hidden engine alongside the app, and two
    // engines each making one would race (the pairing used one, storage kept the other).
    final hub = Hub('${dir.path}/pairing.bin', key == null ? Uint8List(0) : base64Decode(key));
    // Listen to the relay only while on screen. A push to a closed app starts a hidden
    // engine in the same process; if it listened too, it would take each message first
    // and the visible app would drop it as already seen (they share the replay record).
    // Hidden engines handle the push itself (Push._onMessage) and nothing else.
    void maybeStart(AppLifecycleState? s) {
      if (s == AppLifecycleState.resumed && hub.paired && hub._sub == null) hub.start();
    }

    maybeStart(WidgetsBinding.instance.lifecycleState);
    hub._lifecycle = AppLifecycleListener(onStateChange: maybeStart);
    return hub;
  }

  bool get paired => stateKey.isNotEmpty && rust.isPaired(statePath: statePath, stateKey: stateKey);

  List<Child>? children; // null = no status yet
  final Map<String, rust.Rules> rules = {}; // by Linux username
  final Map<String, rust.ChildHistory> history = {};

  /// A message from the PC for the parent (e.g. a refused rule change). Shown once.
  String? notice;
  final List<TimeRequest> requests = [];
  String? problem;
  StreamSubscription<rust.Msg>? _sub;
  AppLifecycleListener? _lifecycle;

  bool get live => children != null;

  DateTime _started = DateTime.now();

  void start() {
    _started = DateTime.now();
    debugPrint('hearth: listening');
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

  /// Messages the PC sent since the app last heard from the relay.
  Future<List<rust.Msg>> poll() => rust.poll(statePath: statePath, stateKey: stateKey);

  /// Pull-to-refresh: ask the PC for everything and wait (up to 10 s) for its status.
  /// Also restarts the relay listener in case it stalled. False if the PC didn't answer.
  Future<bool> refresh() async {
    if (!paired) return false;
    final got = Completer<bool>();
    void onChange() {
      if (!got.isCompleted && live && _statusAt.isAfter(_asked)) got.complete(true);
    }

    _asked = DateTime.now();
    addListener(onChange);
    try {
      start();
      return await got.future.timeout(const Duration(seconds: 10), onTimeout: () => false);
    } finally {
      removeListener(onChange);
    }
  }

  DateTime _asked = DateTime.now();
  DateTime _statusAt = DateTime.fromMillisecondsSinceEpoch(0);

  Future<rust.Msg?> openPush(List<int> content) =>
      rust.openPush(statePath: statePath, stateKey: stateKey, payload: Uint8List.fromList(content));

  /// Tell the PC where to send wake-ups (null = stop).
  Future<void> setPush(String? endpoint) async {
    if (!paired) return;
    try {
      await rust.setPush(statePath: statePath, stateKey: stateKey, endpoint: endpoint);
    } catch (e) {
      // Offline: UnifiedPush hands the endpoint over again on the next start, and we retry then.
      debugPrint('hearth: push endpoint not sent: $e');
    }
  }

  void _on(rust.Msg m) {
    switch (m) {
      case rust.Msg_Status(:final children):
        if (this.children == null) {
          debugPrint('hearth: first status after ${DateTime.now().difference(_started).inMilliseconds} ms');
        }
        this.children = [for (final (i, k) in children.indexed) _child(i, k)];
        _statusAt = DateTime.now();
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
    if (stateKey.isEmpty) {
      // Only the visible app pairs, so this is the one place a key is made.
      final r = Random.secure();
      final k = Uint8List.fromList(List.generate(32, (_) => r.nextInt(256)));
      await _store.write(key: 'pairing_key', value: base64Encode(k));
      stateKey = k;
    }
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
    _lifecycle?.dispose();
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

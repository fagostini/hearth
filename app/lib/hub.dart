import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/painting.dart';
import 'package:path_provider/path_provider.dart';

import 'model.dart';
import 'src/rust/api.dart' as rust;

/// Live state from the paired PC. Until a phone is paired, or before the first status
/// arrives, the screens fall back to demo data (and say so).
class Hub extends ChangeNotifier {
  Hub(this.statePath);

  /// Pairing file in the app's private storage.
  final String statePath;

  static Future<Hub> open() async {
    final dir = await getApplicationSupportDirectory();
    final hub = Hub('${dir.path}/pairing.json');
    if (hub.paired) hub.start();
    return hub;
  }

  bool get paired => rust.isPaired(statePath: statePath);

  List<Child>? children; // null = no status yet
  final List<TimeRequest> requests = [];
  String? problem;
  StreamSubscription<rust.Msg>? _sub;

  bool get live => children != null;

  void start() {
    _sub?.cancel();
    _sub = rust
        .listen(statePath: statePath)
        .listen(
          _on,
          onError: (Object e) {
            problem = '$e';
            notifyListeners();
          },
        );
    // Ask for a fresh status right away (the PC only sends one on changes otherwise).
    rust.requestStatus(statePath: statePath).catchError((Object e) {
      problem = '$e';
      notifyListeners();
    });
  }

  void _on(rust.Msg m) {
    switch (m) {
      case rust.Msg_Status(:final children):
        this.children = [for (final (i, k) in children.indexed) _child(i, k)];
        problem = null;
      case rust.Msg_Request(:final id, :final user, :final minutes, :final reason):
        final kid = children?.where((c) => c.name == user).firstOrNull ?? _child(0, null, name: user);
        requests.removeWhere((r) => r.id == id);
        requests.insert(
          0,
          TimeRequest(id: id, child: kid, minutes: minutes, reason: reason, lockedAt: '', bedtime: ''),
        );
      case rust.Msg_Problem(:final text):
        problem = text;
    }
    notifyListeners();
  }

  /// ponytail: fixed name; a "name this phone" field when there are settings.
  Future<void> pair(String code) async {
    await rust.pair(statePath: statePath, qr: code, phoneName: "Parent's phone");
    start();
    notifyListeners();
  }

  /// Answer a request; removes it from the list once sent.
  Future<void> answer(TimeRequest r, {required bool approve, required int minutes}) async {
    await rust.answer(statePath: statePath, id: r.id, approve: approve, minutes: minutes);
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
      name: k?.user ?? name ?? '?',
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

  /// Signature colours per child, in order: (colour, tint, ink on tint).
  static const palette = [
    (Color(0xFFE8622C), Color(0xFFFBE9E0), Color(0xFFA33F14)),
    (Color(0xFF1F9A8A), Color(0xFFE2F3F0), Color(0xFF0F5F55)),
    (Color(0xFF5B6BE0), Color(0xFFE7E9FB), Color(0xFF3442A8)),
    (Color(0xFFC2457A), Color(0xFFF8E4EC), Color(0xFF8E2A55)),
  ];
}

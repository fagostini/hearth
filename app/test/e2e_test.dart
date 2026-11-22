// ignore_for_file: avoid_print
// End to end through the app's real code (Hub -> Rust bridge -> ntfy -> hearthd).
// Skipped unless HEARTH_E2E_QR is set. Needs `cargo build --release -p hearth_bridge`.
//   HEARTH_E2E_QR='<payload from sudo hearthd pair --text>' flutter test test/e2e_test.dart
// While it waits for a request, have a child run `hearth-agent --ask 10 <reason>`.
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hearth/hub.dart';
import 'package:hearth/src/rust/api.dart' as rust;
import 'package:hearth/src/rust/frb_generated.dart';

Future<void> until(bool Function() ok, String what, {int seconds = 60}) async {
  for (var i = 0; i < seconds * 4 && !ok(); i++) {
    await Future<void>.delayed(const Duration(milliseconds: 250));
  }
  expect(ok(), isTrue, reason: 'timed out waiting for $what');
}

void main() {
  final qr = Platform.environment['HEARTH_E2E_QR'];
  final qr2 = Platform.environment['HEARTH_E2E_QR2'];
  setUpAll(() async {
    if (qr != null || qr2 != null) {
      await RustLib.init(externalLibrary: ExternalLibrary.open('../target/release/libhearth_bridge.so'));
    }
  });
  test(
    'pair, get status, receive a request, approve it',
    () async {
      final dir = Directory.systemTemp.createTempSync('hearth-e2e');
      final hub = Hub('${dir.path}/pairing.json');
      expect(hub.paired, isFalse);

      await hub.pair(qr!);
      expect(hub.paired, isTrue);
      await until(() => hub.live, 'first status');
      print(
        'STATUS ${hub.children!.map((c) => '${c.name}:${c.usedMin}/${c.limitMin}${c.locked ? ' locked' : ''}').join(', ')}',
      );

      print('READY: ask for time now');
      await until(() => hub.requests.isNotEmpty, 'a request', seconds: 120);
      final r = hub.requests.first;
      print('REQUEST ${r.child.name} +${r.minutes} "${r.reason}"');
      await hub.answer(r, approve: true, minutes: 10);
      expect(hub.requests, isEmpty);
      print('APPROVED 10');
      hub.dispose();
    },
    skip: qr == null ? 'set HEARTH_E2E_QR to run' : false,
    timeout: const Timeout(Duration(minutes: 4)),
  );

  // HEARTH_E2E_QR2='<payload>' flutter test test/e2e_test.dart --name rules
  test(
    'rules and history: read, change, refuse a bad change',
    () async {
      final hub = Hub('${Directory.systemTemp.createTempSync('hearth-e2e').path}/pairing.json');
      await hub.pair(qr2!);
      await until(() => hub.rules.isNotEmpty && hub.history.isNotEmpty, 'rules and history');
      final user = hub.rules.keys.first;
      final before = hub.rules[user]!;
      final h = hub.history[user]!;
      print('RULES $user school=${before.schoolMinutes} weekend=${before.weekendMinutes} web=${before.web?.block}');
      print('HISTORY ${h.days.length} days, today used ${h.days.last.usedS}s extra ${h.days.last.extraS}s');

      final want = rust.Rules(
        user: user,
        schoolMinutes: before.schoolMinutes + 15,
        weekendMinutes: before.weekendMinutes,
        schoolWindow: before.schoolWindow,
        weekendWindow: before.weekendWindow,
        apps: before.apps,
        web: rust.WebRule(safesearch: true, block: ['example.org']),
      );
      hub.rules.remove(user); // so we wait for the PC's answer, not our own copy
      await rust.setRules(statePath: hub.statePath, rules: want);
      await until(() => hub.rules[user]?.schoolMinutes == want.schoolMinutes, 'rules applied');
      expect(hub.rules[user]!.web!.block, ['example.org']);
      print('APPLIED school=${hub.rules[user]!.schoolMinutes} web=${hub.rules[user]!.web!.block}');

      final bad = rust.Rules(
        user: user,
        schoolMinutes: 99999,
        weekendMinutes: 60,
        schoolWindow: null,
        weekendWindow: null,
        apps: const [],
        web: null,
      );
      await rust.setRules(statePath: hub.statePath, rules: bad);
      await until(() => hub.notice != null, 'refusal notice');
      print('NOTICE ${hub.notice}');
      expect(hub.rules[user]!.schoolMinutes, want.schoolMinutes, reason: 'refused change must not apply');
      hub.dispose();
    },
    skip: qr2 == null ? 'set HEARTH_E2E_QR2 to run' : false,
    timeout: const Timeout(Duration(minutes: 3)),
  );
}

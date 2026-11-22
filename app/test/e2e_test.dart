// ignore_for_file: avoid_print
// End to end through the app's real code (Hub -> Rust bridge -> ntfy -> hearthd).
// Skipped unless HEARTH_E2E_QR is set. Needs `cargo build --release -p hearth_bridge`.
//   HEARTH_E2E_QR='<payload from sudo hearthd pair --text>' flutter test test/e2e_test.dart
// While it waits for a request, have a child run `hearth-agent --ask 10 <reason>`.
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hearth/hub.dart';
import 'package:hearth/src/rust/frb_generated.dart';

Future<void> until(bool Function() ok, String what, {int seconds = 60}) async {
  for (var i = 0; i < seconds * 4 && !ok(); i++) {
    await Future<void>.delayed(const Duration(milliseconds: 250));
  }
  expect(ok(), isTrue, reason: 'timed out waiting for $what');
}

void main() {
  final qr = Platform.environment['HEARTH_E2E_QR'];
  test('pair, get status, receive a request, approve it', () async {
    await RustLib.init(externalLibrary: ExternalLibrary.open('../target/release/libhearth_bridge.so'));
    final dir = Directory.systemTemp.createTempSync('hearth-e2e');
    final hub = Hub('${dir.path}/pairing.json');
    expect(hub.paired, isFalse);

    await hub.pair(qr!);
    expect(hub.paired, isTrue);
    await until(() => hub.live, 'first status');
    print('STATUS ${hub.children!.map((c) => '${c.name}:${c.usedMin}/${c.limitMin}${c.locked ? ' locked' : ''}').join(', ')}');

    print('READY: ask for time now');
    await until(() => hub.requests.isNotEmpty, 'a request', seconds: 120);
    final r = hub.requests.first;
    print('REQUEST ${r.child.name} +${r.minutes} "${r.reason}"');
    await hub.answer(r, approve: true, minutes: 10);
    expect(hub.requests, isEmpty);
    print('APPROVED 10');
    hub.dispose();
  }, skip: qr == null ? 'set HEARTH_E2E_QR to run' : false, timeout: const Timeout(Duration(minutes: 4)));
}

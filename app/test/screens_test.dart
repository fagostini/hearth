// Renders every screen at phone size to test/goldens/*.png with the real fonts.
// `flutter test --update-goldens` refreshes the images; `flutter test` then fails if a
// screen changes unexpectedly.
import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hearth/main.dart';
import 'package:hearth/model.dart';
import 'package:hearth/screens.dart';

Future<void> _font(String family, String path) async {
  final loader = FontLoader(family)..addFont(Future.value(ByteData.sublistView(File(path).readAsBytesSync())));
  await loader.load();
}

void main() {
  setUpAll(() async {
    await _font('Inter', 'assets/fonts/Inter.ttf');
    await _font('Bricolage', 'assets/fonts/Bricolage.ttf');
    final sdk = Platform.environment['FLUTTER_ROOT'] ?? '${Platform.environment['HOME']}/flutter';
    await _font('MaterialIcons', '$sdk/bin/cache/artifacts/material_fonts/MaterialIcons-Regular.otf');
  });

  final screens = <String, Widget>{
    'today': const Shell(),
    'week': const Shell(initialTab: 1),
    'rules': const Shell(initialTab: 2),
    'request': RequestScreen(req: Demo.request),
    'pair': const PairScreen(),
  };

  for (final MapEntry(key: name, value: screen) in screens.entries) {
    testWidgets(name, (tester) async {
      tester.view.physicalSize = const Size(1170, 2532);
      tester.view.devicePixelRatio = 3;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(HearthApp(home: screen));
      await tester.pump(const Duration(seconds: 2)); // let the rings finish drawing in
      await expectLater(find.byType(MaterialApp), matchesGoldenFile('goldens/$name.png'));
    });
  }
}

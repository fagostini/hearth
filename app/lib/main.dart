import 'package:material_ui/material_ui.dart';

import 'hub.dart';
import 'screens.dart';
import 'src/rust/frb_generated.dart';

/// Live connection to the paired PC. Null in tests and previews: screens show demo data.
Hub? hub;

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  hub = await Hub.open();
  runApp(const HearthApp());
}

/// Palette and type from the design canvas ("Hearth Parent App").
class C {
  static const ground = Color(0xFFF1F2F8);
  static const surface = Color(0xFFFFFFFF);
  static const ink = Color(0xFF171A2C);
  static const muted = Color(0xFF5D6178);
  static const soft = Color(0xFF3C4058);
  static const line = Color(0xFFE1E3EE);
  static const chip = Color(0xFFE3E5EF);
  static const action = Color(0xFFC4501F); // primary button, white text 4.6:1
  static const night = Color(0xFF171A2C);
  static const nightCard = Color(0xFF232742);
  static const mint = Color(0xFF9BE3D8);
}

FontWeight _w(double weight) => FontWeight.values[((weight ~/ 100).clamp(1, 9)) - 1];

TextStyle display(double size, {double weight = 800, Color color = C.ink, double height = 1.05}) => TextStyle(
  fontFamily: 'Bricolage',
  fontSize: size,
  fontWeight: _w(weight),
  fontVariations: [FontVariation.weight(weight)],
  letterSpacing: size >= 28 ? -0.02 * size : 0,
  height: height,
  color: color,
);

TextStyle body(double size, {double weight = 400, Color color = C.ink, double height = 1.4}) => TextStyle(
  fontFamily: 'Inter',
  fontSize: size,
  fontWeight: _w(weight),
  fontVariations: [FontVariation.weight(weight)],
  height: height,
  color: color,
  fontFeatures: const [FontFeature.tabularFigures()],
);

class HearthApp extends StatelessWidget {
  const HearthApp({super.key, this.home});
  final Widget? home;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Hearth',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        scaffoldBackgroundColor: C.ground,
        colorScheme: ColorScheme.fromSeed(seedColor: C.action, surface: C.surface),
        fontFamily: 'Inter',
        navigationBarTheme: NavigationBarThemeData(
          backgroundColor: C.surface,
          indicatorColor: C.chip,
          labelTextStyle: WidgetStateProperty.resolveWith(
            (s) => body(12, weight: 500, color: s.contains(WidgetState.selected) ? C.ink : C.muted),
          ),
        ),
      ),
      home: home ?? const Shell(),
    );
  }
}

class Shell extends StatefulWidget {
  const Shell({super.key, this.initialTab = 0});
  final int initialTab;

  @override
  State<Shell> createState() => _ShellState();
}

class _ShellState extends State<Shell> {
  late int tab = widget.initialTab;
  @override
  Widget build(BuildContext context) {
    final h = hub;
    if (h != null && !h.paired) {
      return ListenableBuilder(listenable: h, builder: (_, _) => h.paired ? _tabs() : const PairScreen(first: true));
    }
    return _tabs();
  }

  Widget _tabs() {
    return Scaffold(
      body: SafeArea(
        bottom: false,
        child: IndexedStack(index: tab, children: const [TodayScreen(), WeekScreen(), RulesScreen()]),
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: tab,
        onDestinationSelected: (i) => setState(() => tab = i),
        destinations: const [
          NavigationDestination(
            icon: Icon(Icons.schedule_outlined),
            selectedIcon: Icon(Icons.schedule),
            label: 'Today',
          ),
          NavigationDestination(
            icon: Icon(Icons.bar_chart_outlined),
            selectedIcon: Icon(Icons.bar_chart),
            label: 'Week',
          ),
          NavigationDestination(icon: Icon(Icons.tune_outlined), selectedIcon: Icon(Icons.tune), label: 'Rules'),
        ],
      ),
    );
  }
}

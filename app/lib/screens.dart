import 'dart:math' as math;

import 'dart:async';
import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:mobile_scanner/mobile_scanner.dart';

import 'hub.dart';
import 'main.dart';
import 'push.dart';
import 'model.dart';
import 'src/rust/api.dart' as rust;

// ---------- shared pieces ----------

class Header extends StatelessWidget {
  const Header({super.key, required this.over, required this.title, this.trailing});
  final String over;
  final String title;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.end,
      children: [
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(over, style: body(13, weight: 500, color: C.muted)),
              const SizedBox(height: 2),
              Semantics(header: true, child: Text(title, style: display(40))),
            ],
          ),
        ),
        ?trailing,
      ],
    );
  }
}

class Panel extends StatelessWidget {
  const Panel({super.key, required this.child, this.color = C.surface, this.padding = 18});
  final Widget child;
  final Color color;
  final double padding;

  @override
  Widget build(BuildContext context) => Material(
    color: color,
    borderRadius: BorderRadius.circular(22),
    child: Padding(padding: EdgeInsets.all(padding), child: child),
  );
}

class Pill extends StatelessWidget {
  const Pill(this.text, {super.key, required this.bg, required this.fg});
  final String text;
  final Color bg, fg;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
    decoration: BoxDecoration(color: bg, borderRadius: BorderRadius.circular(999)),
    child: Text(text, style: body(12, weight: 600, color: fg, height: 1.2)),
  );
}

class Avatar extends StatelessWidget {
  const Avatar(this.child, {super.key, this.size = 44});
  final Child child;
  final double size;

  @override
  Widget build(BuildContext context) => Container(
    width: size,
    height: size,
    alignment: Alignment.center,
    decoration: BoxDecoration(color: child.color, shape: BoxShape.circle),
    child: Text(child.name[0], style: display(size * 0.42, color: Colors.white, height: 1)),
  );
}

/// Pulsing dot: the PC is online. Still when the system asks for reduced motion.
class LiveDot extends StatefulWidget {
  const LiveDot({super.key, required this.color});
  final Color color;
  @override
  State<LiveDot> createState() => _LiveDotState();
}

class _LiveDotState extends State<LiveDot> with SingleTickerProviderStateMixin {
  late final AnimationController c = AnimationController(vsync: this, duration: const Duration(milliseconds: 2400));

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    MediaQuery.of(context).disableAnimations ? c.stop() : c.repeat();
  }

  @override
  void dispose() {
    c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: c,
    builder: (_, _) {
      final t = math.sin(c.value * math.pi);
      return Container(
        width: 8,
        height: 8,
        decoration: BoxDecoration(
          color: widget.color,
          shape: BoxShape.circle,
          boxShadow: [
            BoxShadow(
              color: widget.color.withValues(alpha: 0.45 * (1 - t)),
              spreadRadius: 6 * t,
            ),
          ],
        ),
      );
    },
  );
}

// ---------- the time ring ----------

class Ring extends StatelessWidget {
  const Ring({super.key, required this.child, this.size = 112});
  final Child child;
  final double size;

  @override
  Widget build(BuildContext context) {
    final fraction = child.limitMin == 0 ? 0.0 : child.leftMin / child.limitMin;
    final reduce = MediaQuery.of(context).disableAnimations;
    return Semantics(
      label: child.locked
          ? '${child.name}: no time left, screen locked'
          : '${child.name}: ${child.leftMin} of ${child.limitMin} minutes left',
      excludeSemantics: true,
      child: TweenAnimationBuilder<double>(
        tween: Tween(begin: reduce ? fraction : 0, end: fraction),
        duration: const Duration(milliseconds: 1100),
        curve: Curves.easeOutCubic,
        builder: (_, v, _) => CustomPaint(
          size: Size.square(size),
          painter: _RingPainter(v, child.color, child.tint),
          child: SizedBox.square(
            dimension: size,
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                if (child.locked)
                  Icon(Icons.lock_outline, color: child.ink, size: 30)
                else
                  Text('${child.leftMin}', style: display(size * 0.3, height: 1)),
                const SizedBox(height: 2),
                Text(child.locked ? 'locked' : 'min left', style: body(11, weight: 500, color: C.muted, height: 1.1)),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _RingPainter extends CustomPainter {
  _RingPainter(this.fraction, this.color, this.track);
  final double fraction;
  final Color color, track;

  @override
  void paint(Canvas canvas, Size size) {
    const stroke = 11.0;
    final rect = (Offset.zero & size).deflate(stroke / 2 + 2);
    final p = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = stroke
      ..strokeCap = StrokeCap.round;
    canvas.drawArc(rect, 0, math.pi * 2, false, p..color = track);
    if (fraction > 0) canvas.drawArc(rect, -math.pi / 2, math.pi * 2 * fraction, false, p..color = color);
  }

  @override
  bool shouldRepaint(_RingPainter o) => o.fraction != fraction || o.color != color;
}

// ---------- Today ----------

String _today() {
  const days = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];
  const months = [
    'January',
    'February',
    'March',
    'April',
    'May',
    'June',
    'July',
    'August',
    'September',
    'October',
    'November',
    'December',
  ];
  final n = DateTime.now();
  return '${days[n.weekday - 1]} ${n.day} ${months[n.month - 1]} · ${n.hour.toString().padLeft(2, '0')}:${n.minute.toString().padLeft(2, '0')}';
}

class TodayScreen extends StatelessWidget {
  const TodayScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final h = hub;
    if (h == null) return _build(context, null);
    return ListenableBuilder(listenable: h, builder: (context, _) => _build(context, h));
  }

  Widget _build(BuildContext context, Hub? h) {
    final live = h?.live ?? false;
    final kids = live ? h!.children! : Demo.children;
    final req = live ? h!.requests.firstOrNull : Demo.request;
    return ListView(
      padding: const EdgeInsets.fromLTRB(20, 24, 20, 24),
      children: [
        Header(
          over: live ? _today() : 'Sunday 22 November · 17:46',
          title: 'Today',
          trailing: Material(
            color: C.surface,
            shape: const StadiumBorder(),
            child: InkWell(
              customBorder: const StadiumBorder(),
              onTap: () => _pcSheet(context),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 12),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    LiveDot(color: h?.problem == null ? const Color(0xFF1F9A8A) : C.muted),
                    const SizedBox(width: 8),
                    Text('Family PC', style: body(12, weight: 600, color: const Color(0xFF13776A), height: 1.2)),
                  ],
                ),
              ),
            ),
          ),
        ),
        if (h?.problem case final p?) ...[
          const SizedBox(height: 14),
          Container(
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(color: const Color(0xFFFBE9E0), borderRadius: BorderRadius.circular(16)),
            child: Text(
              'Can\'t reach the PC right now. Retrying. ($p)',
              style: body(13, color: const Color(0xFFA33F14)),
            ),
          ),
        ],
        if (live && Platform.isAndroid && !Push.available) ...[
          const SizedBox(height: 14),
          Container(
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
              border: Border.all(color: C.line),
              borderRadius: BorderRadius.circular(16),
            ),
            child: Text(
              'To hear about requests while Hearth is closed, install the free ntfy app (F-Droid or Play Store), '
              'then reopen Hearth.',
              style: body(13, color: C.muted),
            ),
          ),
        ],
        if (req != null) ...[const SizedBox(height: 18), _RequestCard(req: req)],
        const SizedBox(height: 18),
        if (h != null && h.paired && !live) ...[
          Panel(
            child: Text('Waiting for the first update from the PC…', style: body(14, color: C.muted)),
          ),
          const SizedBox(height: 14),
        ],
        for (final c in kids) ...[
          Panel(
            child: Row(
              children: [
                Ring(child: c),
                const SizedBox(width: 18),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(c.name, style: display(22, weight: 700)),
                      const SizedBox(height: 6),
                      Text(c.note, style: body(13, color: C.muted)),
                      const SizedBox(height: 8),
                      Pill(c.pill, bg: c.tint, fg: c.ink),
                    ],
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 14),
        ],
        // ponytail: demo only until the PC reports filtering stats.
        if (!live)
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
            decoration: BoxDecoration(
              border: Border.all(color: C.line),
              borderRadius: BorderRadius.circular(18),
            ),
            child: Row(
              children: [
                const Icon(Icons.shield_outlined, size: 20, color: C.muted),
                const SizedBox(width: 12),
                Text('SafeSearch on · 4 sites blocked today', style: body(13, color: C.muted)),
              ],
            ),
          ),
        if (!live) ...[
          const SizedBox(height: 14),
          Center(
            child: Pill('Demo data', bg: C.chip, fg: C.soft),
          ),
        ],
      ],
    );
  }
}

/// The "Family PC" chip: pair again, and choose how wake-ups reach this phone.
Future<void> _pcSheet(BuildContext context) async {
  final h = hub;
  final ds = await Push.distributors();
  final cur = await Push.current();
  if (!context.mounted) return;
  await showModalBottomSheet<void>(
    context: context,
    builder: (ctx) => SafeArea(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.all(16),
            child: Text('Family PC', style: display(20, weight: 700)),
          ),
          ListTile(
            leading: const Icon(Icons.qr_code_scanner),
            title: const Text('Pair again'),
            onTap: () {
              Navigator.of(ctx).pop();
              Navigator.of(context).push(MaterialPageRoute(builder: (_) => const PairScreen()));
            },
          ),
          if (ds.isNotEmpty && h != null) ...[
            const Divider(),
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 4),
              child: Text('Notifications while Hearth is closed, through:', style: body(13, color: C.muted)),
            ),
            for (final d in ds)
              ListTile(
                leading: Icon(d == cur ? Icons.radio_button_checked : Icons.radio_button_unchecked),
                title: Text(Push.label(d)),
                subtitle: Text(d, style: body(12, color: C.muted)),
                onTap: () async {
                  Navigator.of(ctx).pop();
                  if (d != cur) await Push.use(h, d);
                },
              ),
          ],
          const SizedBox(height: 8),
        ],
      ),
    ),
  );
}

class _RequestCard extends StatelessWidget {
  const _RequestCard({required this.req});
  final TimeRequest req;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: C.night,
      borderRadius: BorderRadius.circular(22),
      child: InkWell(
        borderRadius: BorderRadius.circular(22),
        onTap: () => Navigator.of(context).push(MaterialPageRoute(builder: (_) => RequestScreen(req: req))),
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Avatar(req.child),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text('ASKING NOW', style: body(12, weight: 600, color: C.mint).copyWith(letterSpacing: 0.7)),
                        Text(
                          '${req.child.name} wants ${req.minutes} more minutes',
                          style: display(19, weight: 700, color: Colors.white, height: 1.2),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              Text('“${req.reason}”', style: body(14, color: const Color(0xFFD7DAE8))),
              const SizedBox(height: 12),
              Container(
                constraints: const BoxConstraints(minHeight: 44),
                padding: const EdgeInsets.symmetric(horizontal: 18),
                decoration: BoxDecoration(color: Colors.white, borderRadius: BorderRadius.circular(999)),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text('Answer', style: body(14, weight: 600)),
                    const SizedBox(width: 6),
                    const Icon(Icons.arrow_forward, size: 16, color: C.ink),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

// ---------- More time request ----------

class RequestScreen extends StatefulWidget {
  const RequestScreen({super.key, required this.req});
  final TimeRequest req;
  @override
  State<RequestScreen> createState() => _RequestScreenState();
}

class _RequestScreenState extends State<RequestScreen> {
  late int minutes = widget.req.minutes;
  bool? approved;
  bool sending = false;

  Future<void> _send(bool approve) async {
    final h = hub;
    if (h == null || widget.req.id == 'demo') return setState(() => approved = approve);
    setState(() => sending = true);
    try {
      await h.answer(widget.req, approve: approve, minutes: minutes);
      if (mounted) setState(() => approved = approve);
    } catch (e) {
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Not sent: $e. Try again.')));
    } finally {
      if (mounted) setState(() => sending = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final r = widget.req, c = r.child;
    return Scaffold(
      appBar: AppBar(
        backgroundColor: C.ground,
        surfaceTintColor: Colors.transparent,
        title: Text('Request', style: body(16, weight: 600)),
      ),
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 8, 20, 20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Center(
                child: Container(
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    border: Border.all(color: c.tint, width: 8),
                  ),
                  child: Avatar(c, size: 84),
                ),
              ),
              const SizedBox(height: 14),
              Text(
                '${c.name} wants ${r.minutes} more minutes',
                textAlign: TextAlign.center,
                style: display(30, height: 1.1),
              ),
              const SizedBox(height: 14),
              Center(
                child: Container(
                  constraints: const BoxConstraints(maxWidth: 300),
                  padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 14),
                  decoration: const BoxDecoration(
                    color: C.surface,
                    borderRadius: BorderRadius.only(
                      topLeft: Radius.circular(20),
                      topRight: Radius.circular(20),
                      bottomRight: Radius.circular(20),
                      bottomLeft: Radius.circular(6),
                    ),
                  ),
                  child: Text('“${r.reason}”', style: body(16, height: 1.45)),
                ),
              ),
              const SizedBox(height: 22),
              Row(
                children: [
                  for (final (k, v) in [
                    ('USED', hm(c.usedMin)),
                    ('LOCKED', r.lockedAt),
                    ('BEDTIME', r.bedtime),
                  ].where((e) => e.$2.isNotEmpty)) ...[
                    Expanded(
                      child: Panel(
                        padding: 12,
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(k, style: body(11, weight: 600, color: C.muted).copyWith(letterSpacing: 0.66)),
                            const SizedBox(height: 4),
                            Text(v, style: body(15, weight: 600)),
                          ],
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                  ],
                ],
              ),
              const SizedBox(height: 22),
              AnimatedSwitcher(
                duration: const Duration(milliseconds: 250),
                child: approved == null ? _choose(c) : _done(c),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _choose(Child c) => Column(
    key: const ValueKey('choose'),
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Text('How much?', style: body(13, weight: 600, color: C.muted)),
      const SizedBox(height: 12),
      Row(
        children: [
          for (final n in [10, 15, 30]) ...[
            Expanded(
              child: Semantics(
                selected: n == minutes,
                button: true,
                child: OutlinedButton(
                  onPressed: () => setState(() => minutes = n),
                  style: OutlinedButton.styleFrom(
                    minimumSize: const Size.fromHeight(52),
                    backgroundColor: n == minutes ? C.ink : C.surface,
                    foregroundColor: n == minutes ? Colors.white : C.ink,
                    side: BorderSide(color: n == minutes ? C.ink : C.line, width: 2),
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
                    padding: const EdgeInsets.symmetric(horizontal: 4),
                    textStyle: body(16, weight: 600),
                  ),
                  child: Text('+$n min'),
                ),
              ),
            ),
            if (n != 30) const SizedBox(width: 8),
          ],
        ],
      ),
      const SizedBox(height: 28),
      FilledButton(
        onPressed: sending ? null : () => _send(true),
        style: FilledButton.styleFrom(
          minimumSize: const Size.fromHeight(56),
          backgroundColor: C.action,
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(18)),
          textStyle: body(17, weight: 600),
        ),
        child: Text('Give $minutes minutes'),
      ),
      const SizedBox(height: 6),
      TextButton(
        onPressed: sending ? null : () => _send(false),
        style: TextButton.styleFrom(
          minimumSize: const Size.fromHeight(48),
          foregroundColor: C.ink,
          textStyle: body(15, weight: 600),
        ),
        child: const Text('Not today'),
      ),
    ],
  );

  Widget _done(Child c) => Column(
    key: const ValueKey('done'),
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Semantics(
        liveRegion: true,
        child: Panel(
          color: C.night,
          padding: 22,
          child: Column(
            children: [
              const Icon(Icons.check_circle_outline, color: C.mint, size: 36),
              const SizedBox(height: 10),
              Text(
                approved! ? 'Sent to the PC' : 'Told ${c.name} no',
                style: display(20, weight: 700, color: Colors.white),
              ),
              const SizedBox(height: 8),
              Text(
                approved!
                    ? c.locked
                          ? '${c.name} can unlock the screen now and has $minutes more minutes today.'
                          : '${c.name} now has $minutes more minutes today.'
                    : '${c.name} sees “Your parent said no this time.” The screen stays locked.',
                textAlign: TextAlign.center,
                style: body(14, color: const Color(0xFFD7DAE8), height: 1.45),
              ),
            ],
          ),
        ),
      ),
      const SizedBox(height: 16),
      TextButton(
        onPressed: () => Navigator.of(context).maybePop(),
        style: TextButton.styleFrom(
          minimumSize: const Size.fromHeight(48),
          backgroundColor: C.chip,
          foregroundColor: C.ink,
          textStyle: body(15, weight: 600),
        ),
        child: const Text('Done'),
      ),
    ],
  );
}

// ---------- Week ----------

/// Pick one child. `names` are display names, `inks` their text colours.
class ChildSwitch extends StatelessWidget {
  const ChildSwitch({
    super.key,
    required this.names,
    required this.inks,
    required this.selected,
    required this.onChanged,
  });
  final List<String> names;
  final List<Color> inks;
  final int selected;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(4),
    decoration: BoxDecoration(color: C.chip, borderRadius: BorderRadius.circular(999)),
    child: Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final (i, n) in names.indexed)
          Semantics(
            selected: i == selected,
            button: true,
            child: TextButton(
              onPressed: () => onChanged(i),
              style: TextButton.styleFrom(
                minimumSize: const Size(56, 36),
                backgroundColor: i == selected ? C.surface : Colors.transparent,
                foregroundColor: i == selected ? inks[i] : C.soft,
                shape: const StadiumBorder(),
                textStyle: body(13, weight: 600),
              ),
              child: Text(n),
            ),
          ),
      ],
    ),
  );
}

/// "org.kde.konsole" -> "Konsole", "firefox_firefox" -> "Firefox".
String appName(String id) {
  final last = id.split('.').last.split('_').first;
  return last.isEmpty ? id : last[0].toUpperCase() + last.substring(1);
}

/// One child's week, from the PC or the demo.
class WeekData {
  WeekData(this.user, this.days, this.apps);
  final String user;
  final List<Day> days;
  final List<AppUse> apps;

  static WeekData fromPc(rust.ChildHistory h) {
    const letters = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];
    final days = [
      for (final (i, d) in h.days.indexed)
        Day(
          letters[(DateTime.tryParse(d.date)?.weekday ?? 1) - 1],
          d.usedS.toInt() ~/ 60,
          d.extraS.toInt() ~/ 60,
          d.limitS.toInt() ~/ 60,
          today: i == h.days.length - 1,
        ),
    ];
    return WeekData(h.user, days, [for (final a in h.apps) AppUse(appName(a.app), a.seconds.toInt() ~/ 60)]);
  }
}

class WeekScreen extends StatefulWidget {
  const WeekScreen({super.key});
  @override
  State<WeekScreen> createState() => _WeekScreenState();
}

class _WeekScreenState extends State<WeekScreen> {
  int who = 0;

  @override
  void initState() {
    super.initState();
    hub?.refreshHistory().catchError((Object _) {});
  }

  @override
  Widget build(BuildContext context) {
    final h = hub;
    if (h == null) return _build(null);
    return ListenableBuilder(listenable: h, builder: (_, _) => _build(h));
  }

  Widget _build(Hub? h) {
    final live = h != null && h.history.isNotEmpty;
    final weeks = live
        ? [for (final x in h.history.values) WeekData.fromPc(x)]
        : [for (final c in Demo.children) WeekData(c.name, Demo.week, Demo.apps)];
    final colours = [
      for (final (i, w) in weeks.indexed)
        live ? h.paletteFor(w.user) : (Demo.children[i].color, Demo.children[i].tint, Demo.children[i].ink),
    ];
    final i = who.clamp(0, weeks.length - 1);
    final week = weeks[i];
    final (color, tint, ink) = colours[i];
    final total = week.days.fold(0, (s, d) => s + d.used);
    final maxApp = week.apps.fold(1, (m, a) => math.max(m, a.minutes));
    final barTint = Color.lerp(color, Colors.white, 0.55)!;
    return ListView(
      padding: const EdgeInsets.fromLTRB(20, 24, 20, 24),
      children: [
        Header(
          over: live ? 'Last 7 days' : 'Demo · 16–22 November',
          title: 'Week',
          trailing: weeks.length < 2
              ? null
              : ChildSwitch(
                  names: [for (final w in weeks) live ? Hub.display(w.user) : w.user],
                  inks: [for (final c in colours) c.$3],
                  selected: i,
                  onChanged: (n) => setState(() => who = n),
                ),
        ),
        const SizedBox(height: 18),
        Panel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (weeks.length < 2)
                Text(live ? Hub.display(week.user) : week.user, style: display(18, weight: 700, color: ink)),
              Wrap(
                crossAxisAlignment: WrapCrossAlignment.end,
                spacing: 10,
                children: [
                  Text(hm(total), style: display(34)),
                  Padding(
                    padding: const EdgeInsets.only(bottom: 4),
                    child: Text('this week · ${hm((total / 7).round())} a day', style: body(13, color: C.muted)),
                  ),
                ],
              ),
              const SizedBox(height: 14),
              WeekChart(days: week.days, color: color, tint: barTint),
              const SizedBox(height: 12),
              Wrap(
                spacing: 16,
                runSpacing: 6,
                children: [
                  _legend(barTint, 'Used'),
                  _legend(color, 'Extra time used'),
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Container(width: 14, height: 2, color: C.ink),
                      const SizedBox(width: 6),
                      Text('Limit', style: body(12, color: C.muted)),
                    ],
                  ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: 14),
        Panel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Apps', style: display(18, weight: 700)),
              if (week.apps.isEmpty) ...[
                const SizedBox(height: 8),
                Text('Only apps with a limit are counted. Add one under Rules.', style: body(14, color: C.muted)),
              ],
              for (final a in week.apps) ...[
                const SizedBox(height: 12),
                Row(
                  children: [
                    Expanded(child: Text(a.name, style: body(14, weight: 600))),
                    Text(hm(a.minutes), style: body(14, color: C.soft)),
                  ],
                ),
                const SizedBox(height: 6),
                ClipRRect(
                  borderRadius: BorderRadius.circular(4),
                  child: LinearProgressIndicator(
                    value: a.minutes / maxApp,
                    minHeight: 8,
                    color: color,
                    backgroundColor: tint,
                  ),
                ),
              ],
            ],
          ),
        ),
        if (!live) ...[
          const SizedBox(height: 14),
          Panel(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text('Blocked sites', style: display(18, weight: 700)),
                const SizedBox(height: 8),
                Text('23 attempts this week: tiktok.com 19, roblox.com 4', style: body(14, color: C.soft, height: 1.5)),
              ],
            ),
          ),
        ],
      ],
    );
  }

  Widget _legend(Color color, String label) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      Container(
        width: 12,
        height: 12,
        decoration: BoxDecoration(color: color, borderRadius: BorderRadius.circular(3)),
      ),
      const SizedBox(width: 6),
      Text(label, style: body(12, color: C.muted)),
    ],
  );
}

class WeekChart extends StatelessWidget {
  const WeekChart({super.key, required this.days, required this.color, required this.tint});
  final List<Day> days;
  final Color color, tint;
  static const h = 160.0;

  @override
  Widget build(BuildContext context) {
    // Scale to the week's biggest bar or limit, at least 3 h; grid every 1, 2 or 4 h.
    final peak = days.fold(180, (m, d) => math.max(m, math.max(d.used, d.limit + d.extra)));
    final hours = (peak / 60).ceil();
    final step = hours <= 4 ? 1 : (hours <= 8 ? 2 : 4);
    final ticks = [for (var t = step; t <= hours; t += step) t];
    final maxMin = hours * 60.0;
    double y(int m) => m / maxMin * h;
    return Semantics(
      label: 'Minutes per day: ${days.map((d) => '${d.label} ${d.used}').join(', ')}',
      excludeSemantics: true,
      child: SizedBox(
        height: h + 22,
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SizedBox(
              width: 28,
              height: h,
              child: Stack(
                clipBehavior: Clip.none,
                children: [
                  for (final hr in ticks)
                    Positioned(
                      right: 4,
                      bottom: y(hr * 60) - 6,
                      child: Text('$hr h', style: body(10, color: C.muted, height: 1)),
                    ),
                ],
              ),
            ),
            Expanded(
              child: Stack(
                clipBehavior: Clip.none,
                children: [
                  for (final hr in ticks)
                    Positioned(
                      left: 0,
                      right: 0,
                      top: h - y(hr * 60),
                      child: Container(height: 1, color: C.line),
                    ),
                  Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      for (final d in days)
                        Expanded(
                          child: Padding(
                            padding: const EdgeInsets.symmetric(horizontal: 4),
                            child: SizedBox(
                              height: h + 22,
                              child: Stack(
                                clipBehavior: Clip.none,
                                alignment: Alignment.bottomCenter,
                                children: [
                                  Positioned(
                                    bottom: 22,
                                    left: 0,
                                    right: 0,
                                    child: Column(
                                      mainAxisSize: MainAxisSize.min,
                                      children: [
                                        if (d.over > 0)
                                          Container(
                                            height: y(d.over),
                                            decoration: BoxDecoration(
                                              color: color,
                                              borderRadius: const BorderRadius.vertical(top: Radius.circular(8)),
                                            ),
                                          ),
                                        Container(
                                          height: y(d.used - d.over),
                                          decoration: BoxDecoration(
                                            color: d.today ? null : tint,
                                            gradient: d.today
                                                ? LinearGradient(
                                                    begin: Alignment.topLeft,
                                                    end: const Alignment(-0.6, -0.8),
                                                    colors: [color, tint],
                                                    stops: const [0.5, 0.5],
                                                    tileMode: TileMode.repeated,
                                                  )
                                                : null,
                                            borderRadius: BorderRadius.vertical(
                                              top: Radius.circular(d.over > 0 ? 0 : 8),
                                              bottom: const Radius.circular(4),
                                            ),
                                          ),
                                        ),
                                      ],
                                    ),
                                  ),
                                  Positioned(
                                    bottom: 22 + y(d.limit) - 1,
                                    left: -3,
                                    right: -3,
                                    child: Container(height: 2, color: C.ink),
                                  ),
                                  Positioned(
                                    bottom: 0,
                                    child: Text(
                                      d.label,
                                      style: body(
                                        11,
                                        weight: d.today ? 700 : 500,
                                        color: d.today ? C.ink : C.muted,
                                        height: 1.4,
                                      ),
                                    ),
                                  ),
                                ],
                              ),
                            ),
                          ),
                        ),
                    ],
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ---------- Rules ----------

rust.Rules _copy(
  rust.Rules r, {
  int? school,
  int? weekend,
  String? Function()? schoolWindow,
  String? Function()? weekendWindow,
  List<rust.AppRule>? apps,
  rust.WebRule? Function()? web,
}) => rust.Rules(
  user: r.user,
  schoolMinutes: school ?? r.schoolMinutes,
  weekendMinutes: weekend ?? r.weekendMinutes,
  schoolWindow: schoolWindow != null ? schoolWindow() : r.schoolWindow,
  weekendWindow: weekendWindow != null ? weekendWindow() : r.weekendWindow,
  apps: apps ?? r.apps,
  web: web != null ? web() : r.web,
);

final _demoRules = [
  rust.Rules(
    user: 'Mia',
    schoolMinutes: 120,
    weekendMinutes: 180,
    schoolWindow: '07:00-20:30',
    weekendWindow: '08:00-21:30',
    apps: [
      rust.AppRule(app: 'minecraft', minutes: 60),
      rust.AppRule(app: 'org.vinegarhq.Sober', minutes: 0),
    ],
    web: rust.WebRule(safesearch: true, block: ['tiktok.com', 'roblox.com']),
  ),
  rust.Rules(
    user: 'Leo',
    schoolMinutes: 120,
    weekendMinutes: 150,
    schoolWindow: '07:00-20:00',
    weekendWindow: null,
    apps: [],
    web: null,
  ),
];

class RulesScreen extends StatefulWidget {
  const RulesScreen({super.key});
  @override
  State<RulesScreen> createState() => _RulesScreenState();
}

class _RulesScreenState extends State<RulesScreen> {
  int who = 0;
  final Map<String, rust.Rules> drafts = {}; // edits not yet confirmed by the PC
  Timer? debounce;
  String? saving; // user whose edit is waiting to be sent
  final List<rust.Rules> demo = [..._demoRules];

  @override
  void dispose() {
    debounce?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final h = hub;
    if (h == null) return _build(null);
    return ListenableBuilder(
      listenable: h,
      builder: (context, _) {
        final n = h.notice;
        if (n != null) {
          h.notice = null;
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(n)));
          });
        }
        return _build(h);
      },
    );
  }

  /// Change a child's rules: on screen at once, sent to the PC a moment later so a row
  /// of taps on + becomes one message.
  void _edit(Hub? h, rust.Rules next) {
    setState(() {
      if (h == null || h.rules.isEmpty) {
        demo[demo.indexWhere((r) => r.user == next.user)] = next;
        return;
      }
      drafts[next.user] = next;
      saving = next.user;
    });
    if (h == null || h.rules.isEmpty) return;
    debounce?.cancel();
    debounce = Timer(const Duration(milliseconds: 900), () async {
      final d = drafts.remove(next.user);
      if (d == null) return;
      try {
        await h.saveRules(d);
      } catch (e) {
        if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Not saved: $e')));
      } finally {
        if (mounted) setState(() => saving = null);
      }
    });
  }

  Widget _build(Hub? h) {
    final live = h != null && h.rules.isNotEmpty;
    final all = live ? [for (final r in h.rules.values) drafts[r.user] ?? r] : demo;
    final i = who.clamp(0, all.length - 1);
    final r = all[i];
    final (color, tint, ink) = live
        ? h.paletteFor(r.user)
        : (Demo.children[i].color, Demo.children[i].tint, Demo.children[i].ink);
    final status = !live ? 'Demo · not connected to a PC' : (saving != null ? 'Saving…' : 'Saved on the family PC');
    return ListView(
      padding: const EdgeInsets.fromLTRB(20, 24, 20, 24),
      children: [
        Header(
          over: status,
          title: 'Rules',
          trailing: all.length < 2
              ? null
              : ChildSwitch(
                  names: [for (final x in all) live ? Hub.display(x.user) : x.user],
                  inks: [for (final x in all) live ? h.paletteFor(x.user).$3 : Demo.children[all.indexOf(x)].ink],
                  selected: i,
                  onChanged: (n) => setState(() => who = n),
                ),
        ),
        if (all.length < 2) Text(live ? Hub.display(r.user) : r.user, style: display(18, weight: 700, color: ink)),
        const SizedBox(height: 16),
        _section('Screen time a day', [
          _stepper('School days', r.schoolMinutes, (v) => _edit(h, _copy(r, school: v))),
          const SizedBox(height: 12),
          _stepper('Weekends', r.weekendMinutes, (v) => _edit(h, _copy(r, weekend: v))),
        ]),
        const SizedBox(height: 14),
        _section('Allowed hours', [
          _window('School days', r.schoolWindow, color, (w) => _edit(h, _copy(r, schoolWindow: () => w))),
          const SizedBox(height: 12),
          _window('Weekends', r.weekendWindow, color, (w) => _edit(h, _copy(r, weekendWindow: () => w))),
          const SizedBox(height: 6),
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              for (final t in ['00', '06', '12', '18', '24']) Text(t, style: body(10, color: C.muted)),
            ],
          ),
        ]),
        const SizedBox(height: 14),
        _section('Apps', [
          if (r.apps.isEmpty)
            Text('No app limits. Time in every app counts toward the daily total.', style: body(14, color: C.muted)),
          for (final (n, a) in r.apps.indexed) ...[
            if (n > 0) const Divider(height: 1, color: Color(0xFFEEEFF5)),
            _row(
              appName(a.app),
              ActionChip(
                label: Text(a.minutes == 0 ? 'Blocked' : '${hm(a.minutes)} a day'),
                labelStyle: body(13, weight: 600, color: a.minutes == 0 ? Colors.white : ink),
                backgroundColor: a.minutes == 0 ? C.ink : tint,
                side: BorderSide.none,
                shape: const StadiumBorder(),
                tooltip: 'Change the limit for ${appName(a.app)}',
                onPressed: () => _appLimit(h, r, a),
              ),
            ),
          ],
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton(
              onPressed: () => _addApp(h, r),
              style: TextButton.styleFrom(
                foregroundColor: ink,
                padding: EdgeInsets.zero,
                minimumSize: const Size(0, 44),
                textStyle: body(14, weight: 600),
              ),
              child: const Text('+ Add an app'),
            ),
          ),
        ]),
        const SizedBox(height: 14),
        _section('Web', [
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: r.web != null,
            onChanged: (on) =>
                _edit(h, _copy(r, web: () => on ? rust.WebRule(safesearch: true, block: const []) : null)),
            activeTrackColor: const Color(0xFF1F9A8A),
            title: Text('Filter websites', style: body(15)),
            subtitle: Text('Blocks sites below and secure DNS tricks', style: body(12, color: C.muted)),
          ),
          if (r.web case final w?) ...[
            const Divider(height: 1, color: Color(0xFFEEEFF5)),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              value: w.safesearch,
              onChanged: (v) => _edit(
                h,
                _copy(
                  r,
                  web: () => rust.WebRule(safesearch: v, block: w.block),
                ),
              ),
              activeTrackColor: const Color(0xFF1F9A8A),
              title: Text('SafeSearch', style: body(15)),
              subtitle: Text('Google, YouTube, Bing, DuckDuckGo', style: body(12, color: C.muted)),
            ),
            const Divider(height: 1, color: Color(0xFFEEEFF5)),
            const SizedBox(height: 10),
            Text('Blocked sites', style: body(15)),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final d in w.block)
                  InputChip(
                    label: Text(d),
                    labelStyle: body(13, weight: 500),
                    deleteButtonTooltipMessage: 'Unblock $d',
                    onDeleted: () => _edit(
                      h,
                      _copy(
                        r,
                        web: () => rust.WebRule(safesearch: w.safesearch, block: [...w.block]..remove(d)),
                      ),
                    ),
                  ),
                ActionChip(
                  avatar: const Icon(Icons.add, size: 18),
                  label: const Text('Block a site'),
                  onPressed: () => _addSite(h, r, w),
                ),
              ],
            ),
          ],
        ]),
      ],
    );
  }

  Future<void> _appLimit(Hub? h, rust.Rules r, rust.AppRule a) async {
    const choices = [(30, '30 min a day'), (60, '1 h a day'), (120, '2 h a day'), (0, 'Blocked'), (-1, 'Remove limit')];
    final pick = await showModalBottomSheet<int>(
      context: context,
      builder: (ctx) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Padding(
              padding: const EdgeInsets.all(16),
              child: Text(appName(a.app), style: display(20, weight: 700)),
            ),
            for (final (m, label) in choices)
              ListTile(title: Text(label), selected: m == a.minutes, onTap: () => Navigator.of(ctx).pop(m)),
          ],
        ),
      ),
    );
    if (pick == null) return;
    final apps = [
      for (final x in r.apps)
        if (x.app != a.app) x else if (pick >= 0) rust.AppRule(app: x.app, minutes: pick),
    ];
    _edit(h, _copy(r, apps: apps));
  }

  Future<String?> _ask(String title, String label, String hint) {
    final field = TextEditingController();
    return showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(title),
        content: TextField(
          controller: field,
          autofocus: true,
          autocorrect: false,
          decoration: InputDecoration(labelText: label, hintText: hint, border: const OutlineInputBorder()),
          onSubmitted: (v) => Navigator.of(ctx).pop(v),
        ),
        actions: [
          TextButton(onPressed: () => Navigator.of(ctx).pop(), child: const Text('Cancel')),
          FilledButton(onPressed: () => Navigator.of(ctx).pop(field.text), child: const Text('Add')),
        ],
      ),
    );
  }

  /// ponytail: typed desktop app ID; a list of the PC's installed apps would be friendlier.
  Future<void> _addApp(Hub? h, rust.Rules r) async {
    final id = (await _ask('Add an app', 'App ID on the PC', 'org.mozilla.firefox'))?.trim();
    if (id == null || id.isEmpty || r.apps.any((a) => a.app == id)) return;
    _edit(
      h,
      _copy(
        r,
        apps: [
          ...r.apps,
          rust.AppRule(app: id, minutes: 60),
        ],
      ),
    );
  }

  Future<void> _addSite(Hub? h, rust.Rules r, rust.WebRule w) async {
    var d = (await _ask('Block a site', 'Website', 'example.com'))?.trim().toLowerCase();
    if (d == null || d.isEmpty) return;
    d = d.replaceFirst(RegExp(r'^https?://'), '').replaceFirst(RegExp(r'^www\.'), '').split('/').first;
    if (w.block.contains(d)) return;
    _edit(
      h,
      _copy(
        r,
        web: () => rust.WebRule(safesearch: w.safesearch, block: [...w.block, d!]),
      ),
    );
  }

  Widget _section(String title, List<Widget> children) => Panel(
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(title, style: display(18, weight: 700)),
        const SizedBox(height: 10),
        ...children,
      ],
    ),
  );

  Widget _row(String label, Widget trailing) => ConstrainedBox(
    constraints: const BoxConstraints(minHeight: 48),
    child: Row(
      children: [
        Expanded(child: Text(label, style: body(15))),
        trailing,
      ],
    ),
  );

  Widget _stepper(String label, int value, ValueChanged<int> set) => Row(
    children: [
      Expanded(child: Text(label, style: body(15))),
      IconButton.filledTonal(
        tooltip: '15 minutes less, $label',
        onPressed: value >= 15 ? () => set(value - 15) : null,
        icon: const Icon(Icons.remove),
        style: IconButton.styleFrom(
          backgroundColor: const Color(0xFFEEEFF5),
          foregroundColor: C.ink,
          minimumSize: const Size(44, 44),
        ),
      ),
      SizedBox(
        width: 72,
        child: Text(value == 0 ? 'None' : hm(value), textAlign: TextAlign.center, style: body(17, weight: 600)),
      ),
      IconButton.filledTonal(
        tooltip: '15 minutes more, $label',
        onPressed: value <= 1440 - 15 ? () => set(value + 15) : null,
        icon: const Icon(Icons.add),
        style: IconButton.styleFrom(
          backgroundColor: const Color(0xFFEEEFF5),
          foregroundColor: C.ink,
          minimumSize: const Size(44, 44),
        ),
      ),
    ],
  );

  static double _hours(String hhmm) {
    final p = hhmm.split(':');
    return int.parse(p[0]) + int.parse(p[1]) / 60;
  }

  static String _fmt(TimeOfDay t) => '${t.hour.toString().padLeft(2, '0')}:${t.minute.toString().padLeft(2, '0')}';

  /// Tap to change: pick a start and an end, or clear to allow any time.
  Widget _window(String label, String? window, Color color, ValueChanged<String?> set) {
    final parts = window?.split('-');
    final from = parts == null ? 0.0 : _hours(parts[0]), to = parts == null ? 24.0 : _hours(parts[1]);
    return InkWell(
      borderRadius: BorderRadius.circular(8),
      onTap: () async {
        TimeOfDay t(String s) => TimeOfDay(hour: int.parse(s.split(':')[0]), minute: int.parse(s.split(':')[1]));
        final start = await showTimePicker(
          context: context,
          helpText: '$label: allowed from',
          initialTime: parts == null ? const TimeOfDay(hour: 7, minute: 0) : t(parts[0]),
        );
        if (start == null || !mounted) return;
        final end = await showTimePicker(
          context: context,
          helpText: '$label: allowed until',
          initialTime: parts == null ? const TimeOfDay(hour: 20, minute: 0) : t(parts[1]),
        );
        if (end == null) return;
        if (end.hour * 60 + end.minute <= start.hour * 60 + start.minute) {
          if (mounted) {
            ScaffoldMessenger.of(context)
                .showSnackBar(const SnackBar(content: Text('The end has to be after the start, on the same day.')));
          }
          return;
        }
        set('${_fmt(start)}-${_fmt(end)}');
      },
      onLongPress: () => set(null),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Expanded(child: Text(label, style: body(14))),
                Text(window == null ? 'Any time' : window.replaceFirst('-', ' – '), style: body(14, weight: 600)),
              ],
            ),
            const SizedBox(height: 6),
            LayoutBuilder(
              builder: (_, box) => Container(
                height: 10,
                decoration: BoxDecoration(color: const Color(0xFFEEEFF5), borderRadius: BorderRadius.circular(5)),
                child: Stack(
                  children: [
                    Positioned(
                      left: box.maxWidth * from / 24,
                      width: box.maxWidth * (to - from) / 24,
                      top: 0,
                      bottom: 0,
                      child: Container(
                        decoration: BoxDecoration(color: color, borderRadius: BorderRadius.circular(5)),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ---------- Pair ----------

class PairScreen extends StatefulWidget {
  const PairScreen({super.key, this.first = false});

  /// Shown as the app's first screen (no back button).
  final bool first;
  @override
  State<PairScreen> createState() => _PairScreenState();
}

class _PairScreenState extends State<PairScreen> with SingleTickerProviderStateMixin {
  late final AnimationController scan = AnimationController(vsync: this, duration: const Duration(milliseconds: 2600));

  /// Camera only where the scanner plugin exists (Android, iOS); the desktop app
  /// shows the placeholder and pairs by typing the code.
  final bool camera = Platform.isAndroid || Platform.isIOS;
  late final MobileScannerController? scanner = camera
      ? MobileScannerController(formats: const [BarcodeFormat.qrCode], detectionSpeed: DetectionSpeed.noDuplicates)
      : null;
  bool busy = false;
  String? error;

  Future<void> _onDetect(BarcodeCapture capture) async {
    final code = capture.barcodes.firstOrNull?.rawValue;
    final h = hub;
    if (code == null || busy || h == null) return;
    setState(() {
      busy = true;
      error = null;
    });
    try {
      await h.pair(code);
      if (mounted && Navigator.of(context).canPop()) Navigator.of(context).pop();
    } catch (e) {
      if (mounted) {
        setState(() => error = pairingError(code, e));
      }
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    MediaQuery.of(context).disableAnimations ? scan.stop() : scan.repeat(reverse: true);
  }

  @override
  void dispose() {
    scanner?.dispose();
    scan.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    const ember = Color(0xFFE8622C);
    return Scaffold(
      backgroundColor: C.night,
      resizeToAvoidBottomInset: false, // the code dialog brings its own room for the keyboard
      appBar: AppBar(backgroundColor: C.night, foregroundColor: Colors.white, automaticallyImplyLeading: !widget.first),
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(24, 0, 24, 24),
          child: Column(
            children: [
              Text(
                'Pair with your family PC',
                textAlign: TextAlign.center,
                style: display(32, color: Colors.white, height: 1.1),
              ),
              const SizedBox(height: 10),
              Text.rich(
                TextSpan(
                  style: body(15, color: const Color(0xFFC9CCDD), height: 1.5),
                  children: [
                    const TextSpan(text: 'On the PC, open Hearth and choose '),
                    TextSpan(
                      text: 'Pair a phone',
                      style: body(15, weight: 600, color: Colors.white, height: 1.5),
                    ),
                    const TextSpan(text: '. Then point this camera at the code.'),
                  ],
                ),
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 26),
              Semantics(
                label: 'Camera view, waiting for a pairing code',
                child: Container(
                  width: 260,
                  height: 260,
                  clipBehavior: Clip.antiAlias,
                  decoration: BoxDecoration(color: C.nightCard, borderRadius: BorderRadius.circular(28)),
                  child: Stack(
                    fit: StackFit.expand,
                    children: [
                      if (camera) MobileScanner(controller: scanner, onDetect: _onDetect),
                      AnimatedBuilder(
                        animation: scan,
                        builder: (_, _) =>
                            CustomPaint(painter: _ViewfinderPainter(scan.value, ember, placeholder: !camera)),
                      ),
                    ],
                  ),
                ),
              ),
              const SizedBox(height: 20),
              Text(
                busy ? 'Pairing…' : (error ?? 'Each code works once, for 10 minutes.'),
                textAlign: TextAlign.center,
                style: body(13, color: error == null ? const Color(0xFFC9CCDD) : const Color(0xFFFFB59A)),
              ),
              const Spacer(),
              Container(
                padding: const EdgeInsets.all(16),
                decoration: BoxDecoration(color: C.nightCard, borderRadius: BorderRadius.circular(20)),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Icon(Icons.lock_outline, color: C.mint, size: 22),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Text(
                        'Your phone and the PC talk end to end encrypted. The relay in between only passes sealed messages on and can\'t read names, times or requests.',
                        style: body(13, color: const Color(0xFFDADCE8), height: 1.5),
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 14),
              OutlinedButton(
                onPressed: () => _typeCode(context),
                style: OutlinedButton.styleFrom(
                  minimumSize: const Size.fromHeight(52),
                  foregroundColor: Colors.white,
                  side: const BorderSide(color: Color(0xFF3A3F62)),
                  shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(18)),
                  textStyle: body(15, weight: 600),
                ),
                child: const Text('Type the code instead'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// What to tell the parent when pairing fails. Details go to the log, not the screen.
String pairingError(String code, Object e) {
  debugPrint('pairing failed: $e');
  final t = code.trim();
  if (!t.startsWith('{') || !t.contains('"secret"')) return 'That isn\'t a Hearth pairing code.';
  if (!t.endsWith('}')) return 'Part of the code is missing. Copy all of it, or show a new code on the PC.';
  if ('$e'.contains('relay')) return 'Can\'t reach the relay. Check the phone is online and try again.';
  return 'That code didn\'t work. Show a new code on the PC and try again.';
}

/// Paste the code shown on the PC (`sudo hearthd pair --text` prints it).
Future<void> _typeCode(BuildContext context) async {
  final field = TextEditingController();
  String? error;
  await showDialog<void>(
    context: context,
    builder: (ctx) => StatefulBuilder(
      builder: (ctx, set) => AlertDialog(
        title: const Text('Type the code'),
        content: TextField(
          controller: field,
          autofocus: true,
          minLines: 3,
          maxLines: 6,
          decoration: InputDecoration(
            labelText: 'Pairing code from the PC',
            errorText: error,
            border: const OutlineInputBorder(),
          ),
        ),
        actions: [
          TextButton(onPressed: () => Navigator.of(ctx).pop(), child: const Text('Cancel')),
          FilledButton(
            onPressed: () async {
              final h = hub;
              if (h == null) return Navigator.of(ctx).pop();
              try {
                await h.pair(field.text);
                if (ctx.mounted) Navigator.of(ctx).pop();
                if (context.mounted && Navigator.of(context).canPop()) Navigator.of(context).pop();
              } catch (e) {
                set(() => error = pairingError(field.text, e));
              }
            },
            child: const Text('Pair'),
          ),
        ],
      ),
    ),
  );
}

class _ViewfinderPainter extends CustomPainter {
  _ViewfinderPainter(this.t, this.color, {required this.placeholder});
  final double t;
  final Color color;
  final bool placeholder; // grey square where the camera picture would be

  @override
  void paint(Canvas canvas, Size s) {
    final p = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 4
      ..strokeCap = StrokeCap.round;
    const m = 24.0, l = 42.0, r = 14.0;
    for (final (x, y, dx, dy) in [
      (m, m, 1.0, 1.0),
      (s.width - m, m, -1.0, 1.0),
      (m, s.height - m, 1.0, -1.0),
      (s.width - m, s.height - m, -1.0, -1.0),
    ]) {
      final path = Path()
        ..moveTo(x, y + dy * l)
        ..lineTo(x, y + dy * r)
        ..quadraticBezierTo(x, y, x + dx * r, y)
        ..lineTo(x + dx * l, y);
      canvas.drawPath(path, p);
    }
    if (placeholder) {
      canvas.drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromCenter(center: s.center(Offset.zero), width: 136, height: 136),
          const Radius.circular(10),
        ),
        Paint()..color = Colors.white.withValues(alpha: 0.14),
      );
    }
    final y = 32 + (s.height - 64) * t;
    canvas.drawLine(
      Offset(32, y),
      Offset(s.width - 32, y),
      Paint()
        ..color = color
        ..strokeWidth = 2
        ..maskFilter = const MaskFilter.blur(BlurStyle.normal, 1.5),
    );
  }

  @override
  bool shouldRepaint(_ViewfinderPainter o) => o.t != t;
}

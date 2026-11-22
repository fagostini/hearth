import 'dart:math' as math;

import 'package:material_ui/material_ui.dart';

import 'main.dart';
import 'model.dart';

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
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(over, style: body(13, weight: 500, color: C.muted)),
            const SizedBox(height: 2),
            Semantics(header: true, child: Text(title, style: display(40))),
          ]),
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
              boxShadow: [BoxShadow(color: widget.color.withValues(alpha: 0.45 * (1 - t)), spreadRadius: 6 * t)],
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
      label: child.locked ? '${child.name}: no time left, screen locked' : '${child.name}: ${child.leftMin} of ${child.limitMin} minutes left',
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
            child: Column(mainAxisAlignment: MainAxisAlignment.center, children: [
              if (child.locked)
                Icon(Icons.lock_outline, color: child.ink, size: 30)
              else
                Text('${child.leftMin}', style: display(size * 0.3, height: 1)),
              const SizedBox(height: 2),
              Text(child.locked ? 'locked' : 'min left', style: body(11, weight: 500, color: C.muted, height: 1.1)),
            ]),
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

class TodayScreen extends StatelessWidget {
  const TodayScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final req = Demo.request;
    return ListView(
      padding: const EdgeInsets.fromLTRB(20, 24, 20, 24),
      children: [
        Header(
          over: 'Sunday 22 November · 17:46',
          title: 'Today',
          trailing: Material(
            color: C.surface,
            shape: const StadiumBorder(),
            child: InkWell(
              customBorder: const StadiumBorder(),
              onTap: () => Navigator.of(context).push(MaterialPageRoute(builder: (_) => const PairScreen())),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 12),
                child: Row(mainAxisSize: MainAxisSize.min, children: [
                  const LiveDot(color: Color(0xFF1F9A8A)),
                  const SizedBox(width: 8),
                  Text('Kitchen PC', style: body(12, weight: 600, color: const Color(0xFF13776A), height: 1.2)),
                ]),
              ),
            ),
          ),
        ),
        const SizedBox(height: 18),
        _RequestCard(req: req),
        const SizedBox(height: 18),
        for (final c in Demo.children) ...[
          Panel(
            child: Row(children: [
              Ring(child: c),
              const SizedBox(width: 18),
              Expanded(
                child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                  Text(c.name, style: display(22, weight: 700)),
                  const SizedBox(height: 6),
                  Text(c.note, style: body(13, color: C.muted)),
                  const SizedBox(height: 8),
                  Pill(c.pill, bg: c.tint, fg: c.ink),
                ]),
              ),
            ]),
          ),
          const SizedBox(height: 14),
        ],
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
          decoration: BoxDecoration(border: Border.all(color: C.line), borderRadius: BorderRadius.circular(18)),
          child: Row(children: [
            const Icon(Icons.shield_outlined, size: 20, color: C.muted),
            const SizedBox(width: 12),
            Text('SafeSearch on · 4 sites blocked today', style: body(13, color: C.muted)),
          ]),
        ),
        const SizedBox(height: 14),
        Center(child: Pill('Demo data', bg: C.chip, fg: C.soft)),
      ],
    );
  }
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
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Row(children: [
              Avatar(req.child),
              const SizedBox(width: 12),
              Expanded(
                child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                  Text('ASKING NOW', style: body(12, weight: 600, color: C.mint).copyWith(letterSpacing: 0.7)),
                  Text('${req.child.name} wants ${req.minutes} more minutes', style: display(19, weight: 700, color: Colors.white, height: 1.2)),
                ]),
              ),
            ]),
            const SizedBox(height: 12),
            Text('“${req.reason}”', style: body(14, color: const Color(0xFFD7DAE8))),
            const SizedBox(height: 12),
            Container(
              constraints: const BoxConstraints(minHeight: 44),
              padding: const EdgeInsets.symmetric(horizontal: 18),
              decoration: BoxDecoration(color: Colors.white, borderRadius: BorderRadius.circular(999)),
              child: Row(mainAxisSize: MainAxisSize.min, children: [
                Text('Answer', style: body(14, weight: 600)),
                const SizedBox(width: 6),
                const Icon(Icons.arrow_forward, size: 16, color: C.ink),
              ]),
            ),
          ]),
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

  @override
  Widget build(BuildContext context) {
    final r = widget.req, c = r.child;
    return Scaffold(
      appBar: AppBar(backgroundColor: C.ground, surfaceTintColor: Colors.transparent, title: Text('Request', style: body(16, weight: 600))),
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 8, 20, 20),
          child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            Center(
              child: Container(
                decoration: BoxDecoration(shape: BoxShape.circle, border: Border.all(color: c.tint, width: 8)),
                child: Avatar(c, size: 84),
              ),
            ),
            const SizedBox(height: 14),
            Text('${c.name} wants ${r.minutes} more minutes', textAlign: TextAlign.center, style: display(30, height: 1.1)),
            const SizedBox(height: 14),
            Center(
              child: Container(
                constraints: const BoxConstraints(maxWidth: 300),
                padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 14),
                decoration: const BoxDecoration(
                  color: C.surface,
                  borderRadius: BorderRadius.only(topLeft: Radius.circular(20), topRight: Radius.circular(20), bottomRight: Radius.circular(20), bottomLeft: Radius.circular(6)),
                ),
                child: Text('“${r.reason}”', style: body(16, height: 1.45)),
              ),
            ),
            const SizedBox(height: 22),
            Row(children: [
              for (final (k, v) in [('USED', hm(c.usedMin)), ('LOCKED', r.lockedAt), ('BEDTIME', r.bedtime)]) ...[
                Expanded(
                  child: Panel(
                    padding: 12,
                    child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                      Text(k, style: body(11, weight: 600, color: C.muted).copyWith(letterSpacing: 0.66)),
                      const SizedBox(height: 4),
                      Text(v, style: body(15, weight: 600)),
                    ]),
                  ),
                ),
                if (k != 'BEDTIME') const SizedBox(width: 8),
              ],
            ]),
            const SizedBox(height: 22),
            AnimatedSwitcher(
              duration: const Duration(milliseconds: 250),
              child: approved == null ? _choose(c) : _done(c),
            ),
          ]),
        ),
      ),
    );
  }

  Widget _choose(Child c) => Column(key: const ValueKey('choose'), crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Text('How much?', style: body(13, weight: 600, color: C.muted)),
        const SizedBox(height: 12),
        Row(children: [
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
        ]),
        const SizedBox(height: 28),
        FilledButton(
          onPressed: () => setState(() => approved = true),
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
          onPressed: () => setState(() => approved = false),
          style: TextButton.styleFrom(minimumSize: const Size.fromHeight(48), foregroundColor: C.ink, textStyle: body(15, weight: 600)),
          child: const Text('Not today'),
        ),
      ]);

  Widget _done(Child c) => Column(key: const ValueKey('done'), crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Semantics(
          liveRegion: true,
          child: Panel(
            color: C.night,
            padding: 22,
            child: Column(children: [
              const Icon(Icons.check_circle_outline, color: C.mint, size: 36),
              const SizedBox(height: 10),
              Text(approved! ? 'Sent to Kitchen PC' : 'Told ${c.name} no', style: display(20, weight: 700, color: Colors.white)),
              const SizedBox(height: 8),
              Text(
                approved!
                    ? '${c.name} can unlock the screen now and has $minutes more minutes today.'
                    : '${c.name} sees “Your parent said no this time.” The screen stays locked.',
                textAlign: TextAlign.center,
                style: body(14, color: const Color(0xFFD7DAE8), height: 1.45),
              ),
            ]),
          ),
        ),
        const SizedBox(height: 16),
        TextButton(
          onPressed: () => setState(() => approved = null),
          style: TextButton.styleFrom(minimumSize: const Size.fromHeight(48), backgroundColor: C.chip, foregroundColor: C.ink, textStyle: body(15, weight: 600)),
          child: const Text('Undo'),
        ),
      ]);
}

// ---------- Week ----------

class ChildSwitch extends StatelessWidget {
  const ChildSwitch({super.key, required this.selected, required this.onChanged});
  final int selected;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) => Container(
        padding: const EdgeInsets.all(4),
        decoration: BoxDecoration(color: C.chip, borderRadius: BorderRadius.circular(999)),
        child: Row(mainAxisSize: MainAxisSize.min, children: [
          for (final (i, c) in Demo.children.indexed)
            Semantics(
              selected: i == selected,
              button: true,
              child: TextButton(
                onPressed: () => onChanged(i),
                style: TextButton.styleFrom(
                  minimumSize: const Size(56, 36),
                  backgroundColor: i == selected ? C.surface : Colors.transparent,
                  foregroundColor: i == selected ? c.ink : C.soft,
                  shape: const StadiumBorder(),
                  textStyle: body(13, weight: 600),
                ),
                child: Text(c.name),
              ),
            ),
        ]),
      );
}

class WeekScreen extends StatefulWidget {
  const WeekScreen({super.key});
  @override
  State<WeekScreen> createState() => _WeekScreenState();
}

class _WeekScreenState extends State<WeekScreen> {
  int who = 0;

  @override
  Widget build(BuildContext context) {
    final c = Demo.children[who];
    final total = Demo.week.fold(0, (s, d) => s + d.used + d.extra);
    final maxApp = Demo.apps.first.minutes;
    return ListView(
      padding: const EdgeInsets.fromLTRB(20, 24, 20, 24),
      children: [
        Header(over: '16–22 November', title: 'Week', trailing: ChildSwitch(selected: who, onChanged: (i) => setState(() => who = i))),
        const SizedBox(height: 18),
        Panel(
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Wrap(crossAxisAlignment: WrapCrossAlignment.end, spacing: 10, children: [
              Text(hm(total), style: display(34)),
              Padding(padding: const EdgeInsets.only(bottom: 4), child: Text('this week · ${hm((total / 7).round())} a day', style: body(13, color: C.muted))),
            ]),
            const SizedBox(height: 14),
            WeekChart(days: Demo.week, color: c.color, tint: Color.lerp(c.color, Colors.white, 0.55)!),
            const SizedBox(height: 12),
            Wrap(spacing: 16, runSpacing: 6, children: [
              _legend(Color.lerp(c.color, Colors.white, 0.55)!, 'Used'),
              _legend(c.color, 'Extra time you gave'),
              Row(mainAxisSize: MainAxisSize.min, children: [
                Container(width: 14, height: 2, color: C.ink),
                const SizedBox(width: 6),
                Text('Limit', style: body(12, color: C.muted)),
              ]),
            ]),
          ]),
        ),
        const SizedBox(height: 14),
        Panel(
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text('Apps', style: display(18, weight: 700)),
            for (final a in Demo.apps) ...[
              const SizedBox(height: 12),
              Row(children: [
                Expanded(child: Text(a.name, style: body(14, weight: 600))),
                Text(hm(a.minutes), style: body(14, color: C.soft)),
              ]),
              const SizedBox(height: 6),
              ClipRRect(
                borderRadius: BorderRadius.circular(4),
                child: LinearProgressIndicator(value: a.minutes / maxApp, minHeight: 8, color: c.color, backgroundColor: c.tint),
              ),
            ],
          ]),
        ),
        const SizedBox(height: 14),
        Panel(
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text('Blocked sites', style: display(18, weight: 700)),
            const SizedBox(height: 8),
            Text('23 attempts this week: tiktok.com 19, roblox.com 4', style: body(14, color: C.soft, height: 1.5)),
          ]),
        ),
      ],
    );
  }

  Widget _legend(Color color, String label) => Row(mainAxisSize: MainAxisSize.min, children: [
        Container(width: 12, height: 12, decoration: BoxDecoration(color: color, borderRadius: BorderRadius.circular(3))),
        const SizedBox(width: 6),
        Text(label, style: body(12, color: C.muted)),
      ]);
}

/// Bars to one scale: 180 min = full height. Each bar marks its own day's limit.
class WeekChart extends StatelessWidget {
  const WeekChart({super.key, required this.days, required this.color, required this.tint});
  final List<Day> days;
  final Color color, tint;
  static const maxMin = 180.0, h = 160.0;

  @override
  Widget build(BuildContext context) {
    double y(int m) => m / maxMin * h;
    return Semantics(
      label: 'Minutes per day: ${days.map((d) => '${d.label} ${d.used + d.extra}').join(', ')}',
      excludeSemantics: true,
      child: SizedBox(
        height: h + 22,
        child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          SizedBox(
            width: 28,
            height: h,
            child: Stack(clipBehavior: Clip.none, children: [
              for (final hr in [1, 2, 3])
                Positioned(right: 4, bottom: y(hr * 60) - 6, child: Text('$hr h', style: body(10, color: C.muted, height: 1))),
            ]),
          ),
          Expanded(
            child: Stack(clipBehavior: Clip.none, children: [
              for (final hr in [1, 2, 3])
                Positioned(left: 0, right: 0, top: h - y(hr * 60), child: Container(height: 1, color: C.line)),
              Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
                for (final d in days)
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 4),
                      child: SizedBox(
                        height: h + 22,
                        child: Stack(clipBehavior: Clip.none, alignment: Alignment.bottomCenter, children: [
                          Positioned(
                            bottom: 22,
                            left: 0,
                            right: 0,
                            child: Column(mainAxisSize: MainAxisSize.min, children: [
                              if (d.extra > 0)
                                Container(height: y(d.extra), decoration: BoxDecoration(color: color, borderRadius: const BorderRadius.vertical(top: Radius.circular(8)))),
                              Container(
                                height: y(d.used),
                                decoration: BoxDecoration(
                                  color: d.today ? null : tint,
                                  gradient: d.today
                                      ? LinearGradient(begin: Alignment.topLeft, end: const Alignment(-0.6, -0.8), colors: [color, tint], stops: const [0.5, 0.5], tileMode: TileMode.repeated)
                                      : null,
                                  borderRadius: BorderRadius.vertical(top: Radius.circular(d.extra > 0 ? 0 : 8), bottom: const Radius.circular(4)),
                                ),
                              ),
                            ]),
                          ),
                          Positioned(bottom: 22 + y(d.limit) - 1, left: -3, right: -3, child: Container(height: 2, color: C.ink)),
                          Positioned(bottom: 0, child: Text(d.label, style: body(11, weight: d.today ? 700 : 500, color: d.today ? C.ink : C.muted, height: 1.4))),
                        ]),
                      ),
                    ),
                  ),
              ]),
            ]),
          ),
        ]),
      ),
    );
  }
}

// ---------- Rules ----------

class RulesScreen extends StatefulWidget {
  const RulesScreen({super.key});
  @override
  State<RulesScreen> createState() => _RulesScreenState();
}

class _RulesScreenState extends State<RulesScreen> {
  int who = 0;
  int school = 120, weekend = 180;
  bool safe = true;

  @override
  Widget build(BuildContext context) {
    final c = Demo.children[who];
    return ListView(
      padding: const EdgeInsets.fromLTRB(20, 24, 20, 24),
      children: [
        Header(over: 'Saved on Kitchen PC', title: 'Rules', trailing: ChildSwitch(selected: who, onChanged: (i) => setState(() => who = i))),
        const SizedBox(height: 16),
        _section('Screen time a day', [
          _stepper('School days', school, (v) => setState(() => school = v)),
          const SizedBox(height: 12),
          _stepper('Weekends', weekend, (v) => setState(() => weekend = v)),
        ]),
        const SizedBox(height: 14),
        _section('Allowed hours', [
          _window('School days', '07:00 – 20:30', 7, 20.5, c),
          const SizedBox(height: 12),
          _window('Weekends', '08:00 – 21:30', 8, 21.5, c),
          const SizedBox(height: 6),
          Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
            for (final t in ['00', '06', '12', '18', '24']) Text(t, style: body(10, color: C.muted)),
          ]),
        ]),
        const SizedBox(height: 14),
        _section('Apps', [
          _row('Minecraft', Pill('1 h a day', bg: c.tint, fg: c.ink)),
          const Divider(height: 1, color: Color(0xFFEEEFF5)),
          _row('Firefox', const Pill('No limit', bg: Color(0xFFEEEFF5), fg: C.soft)),
          const Divider(height: 1, color: Color(0xFFEEEFF5)),
          _row('Sober (Roblox)', const Pill('Blocked', bg: C.ink, fg: Colors.white)),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton(onPressed: () {}, style: TextButton.styleFrom(foregroundColor: c.ink, padding: EdgeInsets.zero, minimumSize: const Size(0, 44), textStyle: body(14, weight: 600)), child: const Text('+ Add an app')),
          ),
        ]),
        const SizedBox(height: 14),
        _section('Web', [
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: safe,
            onChanged: (v) => setState(() => safe = v),
            activeTrackColor: const Color(0xFF1F9A8A),
            title: Text('SafeSearch', style: body(15)),
            subtitle: Text('Google, YouTube, Bing, DuckDuckGo', style: body(12, color: C.muted)),
          ),
          const Divider(height: 1, color: Color(0xFFEEEFF5)),
          _row('Blocked sites', Text('tiktok.com, roblox.com', style: body(14, color: C.soft))),
        ]),
      ],
    );
  }

  Widget _section(String title, List<Widget> children) => Panel(
        child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          Text(title, style: display(18, weight: 700)),
          const SizedBox(height: 10),
          ...children,
        ]),
      );

  Widget _row(String label, Widget trailing) => ConstrainedBox(
        constraints: const BoxConstraints(minHeight: 48),
        child: Row(children: [Expanded(child: Text(label, style: body(15))), trailing]),
      );

  Widget _stepper(String label, int value, ValueChanged<int> set) => Row(children: [
        Expanded(child: Text(label, style: body(15))),
        IconButton.filledTonal(
          tooltip: '15 minutes less, $label',
          onPressed: value > 15 ? () => set(value - 15) : null,
          icon: const Icon(Icons.remove),
          style: IconButton.styleFrom(backgroundColor: const Color(0xFFEEEFF5), foregroundColor: C.ink, minimumSize: const Size(44, 44)),
        ),
        SizedBox(width: 72, child: Text(hm(value), textAlign: TextAlign.center, style: body(17, weight: 600))),
        IconButton.filledTonal(
          tooltip: '15 minutes more, $label',
          onPressed: value < 600 ? () => set(value + 15) : null,
          icon: const Icon(Icons.add),
          style: IconButton.styleFrom(backgroundColor: const Color(0xFFEEEFF5), foregroundColor: C.ink, minimumSize: const Size(44, 44)),
        ),
      ]);

  Widget _window(String label, String text, double from, double to, Child c) => Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Row(children: [Expanded(child: Text(label, style: body(14))), Text(text, style: body(14, weight: 600))]),
        const SizedBox(height: 6),
        LayoutBuilder(
          builder: (_, box) => Container(
            height: 10,
            decoration: BoxDecoration(color: const Color(0xFFEEEFF5), borderRadius: BorderRadius.circular(5)),
            child: Stack(children: [
              Positioned(
                left: box.maxWidth * from / 24,
                width: box.maxWidth * (to - from) / 24,
                top: 0,
                bottom: 0,
                child: Container(decoration: BoxDecoration(color: c.color, borderRadius: BorderRadius.circular(5))),
              ),
            ]),
          ),
        ),
      ]);
}

// ---------- Pair ----------

class PairScreen extends StatefulWidget {
  const PairScreen({super.key});
  @override
  State<PairScreen> createState() => _PairScreenState();
}

class _PairScreenState extends State<PairScreen> with SingleTickerProviderStateMixin {
  late final AnimationController scan = AnimationController(vsync: this, duration: const Duration(milliseconds: 2600));

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    MediaQuery.of(context).disableAnimations ? scan.stop() : scan.repeat(reverse: true);
  }

  @override
  void dispose() {
    scan.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    const ember = Color(0xFFE8622C);
    return Scaffold(
      backgroundColor: C.night,
      appBar: AppBar(backgroundColor: C.night, foregroundColor: Colors.white),
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(24, 0, 24, 24),
          child: Column(children: [
            Text('Pair with your family PC', textAlign: TextAlign.center, style: display(32, color: Colors.white, height: 1.1)),
            const SizedBox(height: 10),
            Text.rich(
              TextSpan(style: body(15, color: const Color(0xFFC9CCDD), height: 1.5), children: [
                const TextSpan(text: 'On the PC, open Hearth and choose '),
                TextSpan(text: 'Pair a phone', style: body(15, weight: 600, color: Colors.white, height: 1.5)),
                const TextSpan(text: '. Then point this camera at the code.'),
              ]),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 26),
            Semantics(
              label: 'Camera view, waiting for a pairing code',
              child: Container(
                width: 260,
                height: 260,
                decoration: BoxDecoration(color: C.nightCard, borderRadius: BorderRadius.circular(28)),
                child: AnimatedBuilder(
                  animation: scan,
                  builder: (_, _) => CustomPaint(painter: _ViewfinderPainter(scan.value, ember)),
                ),
              ),
            ),
            const SizedBox(height: 20),
            Text('Each code works once, for 10 minutes.', style: body(13, color: const Color(0xFFC9CCDD))),
            const Spacer(),
            Container(
              padding: const EdgeInsets.all(16),
              decoration: BoxDecoration(color: C.nightCard, borderRadius: BorderRadius.circular(20)),
              child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
                const Icon(Icons.lock_outline, color: C.mint, size: 22),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    'Your phone and the PC talk end to end encrypted. The relay in between only passes sealed messages on and can\'t read names, times or requests.',
                    style: body(13, color: const Color(0xFFDADCE8), height: 1.5),
                  ),
                ),
              ]),
            ),
            const SizedBox(height: 14),
            OutlinedButton(
              onPressed: () {},
              style: OutlinedButton.styleFrom(
                minimumSize: const Size.fromHeight(52),
                foregroundColor: Colors.white,
                side: const BorderSide(color: Color(0xFF3A3F62)),
                shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(18)),
                textStyle: body(15, weight: 600),
              ),
              child: const Text('Type the code instead'),
            ),
          ]),
        ),
      ),
    );
  }
}

class _ViewfinderPainter extends CustomPainter {
  _ViewfinderPainter(this.t, this.color);
  final double t;
  final Color color;

  @override
  void paint(Canvas canvas, Size s) {
    final p = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 4
      ..strokeCap = StrokeCap.round;
    const m = 24.0, l = 42.0, r = 14.0;
    for (final (x, y, dx, dy) in [(m, m, 1.0, 1.0), (s.width - m, m, -1.0, 1.0), (m, s.height - m, 1.0, -1.0), (s.width - m, s.height - m, -1.0, -1.0)]) {
      final path = Path()
        ..moveTo(x, y + dy * l)
        ..lineTo(x, y + dy * r)
        ..quadraticBezierTo(x, y, x + dx * r, y)
        ..lineTo(x + dx * l, y);
      canvas.drawPath(path, p);
    }
    canvas.drawRRect(
      RRect.fromRectAndRadius(Rect.fromCenter(center: s.center(Offset.zero), width: 136, height: 136), const Radius.circular(10)),
      Paint()..color = Colors.white.withValues(alpha: 0.14),
    );
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

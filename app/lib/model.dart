import 'package:flutter/painting.dart';

/// What the app shows. Filled with demo data until the Rust core (hearth-core via
/// flutter_rust_bridge) delivers real Status messages from the paired PC.
class Child {
  Child({
    required this.name,
    required this.color,
    required this.tint,
    required this.ink,
    required this.usedMin,
    required this.limitMin,
    required this.locked,
    required this.note,
    required this.pill,
  });

  final String name;
  final Color color; // signature colour: rings, bars
  final Color tint; // pale version: ring track, pills
  final Color ink; // dark version: text on tint (passes 4.5:1)
  final int usedMin;
  final int limitMin;
  final bool locked;
  final String note; // e.g. "Minecraft open · 38 min today"
  final String pill; // e.g. "Until 20:30"

  int get leftMin => (limitMin - usedMin).clamp(0, limitMin);
}

class TimeRequest {
  TimeRequest({
    this.id = 'demo',
    required this.child,
    required this.minutes,
    required this.reason,
    required this.lockedAt,
    required this.bedtime,
  });
  final String id;
  final Child child;
  final int minutes;
  final String reason;
  final String lockedAt;
  final String bedtime;
}

/// One day in the week view: minutes used, extra minutes approved, limit.
class Day {
  const Day(this.label, this.used, this.extra, this.limit, {this.today = false});
  final String label;
  final int used;
  final int extra;
  final int limit;
  final bool today;
}

class AppUse {
  const AppUse(this.name, this.minutes);
  final String name;
  final int minutes;
}

class Demo {
  static final mia = Child(
    name: 'Mia',
    color: const Color(0xFFE8622C),
    tint: const Color(0xFFFBE9E0),
    ink: const Color(0xFFA33F14),
    usedMin: 95,
    limitMin: 120,
    locked: false,
    note: '1 h 35 m of 2 h used\nMinecraft open · 38 min today',
    pill: 'Until 20:30',
  );
  static final leo = Child(
    name: 'Leo',
    color: const Color(0xFF1F9A8A),
    tint: const Color(0xFFE2F3F0),
    ink: const Color(0xFF0F5F55),
    usedMin: 120,
    limitMin: 120,
    locked: true,
    note: '2 h of 2 h used\nScreen locked at 17:40',
    pill: 'Daily limit reached',
  );
  static final children = [mia, leo];
  static final request = TimeRequest(
    child: leo,
    minutes: 15,
    reason: 'I\'m almost done with my Lego build video.',
    lockedAt: '17:40',
    bedtime: '20:00',
  );
  static const week = [
    Day('M', 104, 0, 120),
    Day('T', 120, 0, 120),
    Day('W', 88, 0, 120),
    Day('T', 120, 15, 120),
    Day('F', 112, 0, 120),
    Day('S', 171, 0, 180),
    Day('S', 95, 0, 180, today: true),
  ];
  static const apps = [AppUse('Minecraft', 250), AppUse('Firefox', 185), AppUse('Krita', 80)];
}

String hm(int minutes) {
  final h = minutes ~/ 60, m = minutes % 60;
  if (h == 0) return '$m min';
  return m == 0 ? '$h h' : '$h h ${m.toString().padLeft(2, '0')} m';
}

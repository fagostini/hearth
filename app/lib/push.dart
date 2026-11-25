import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:material_ui/material_ui.dart' show MaterialPageRoute;
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:unifiedpush/unifiedpush.dart';
import 'package:workmanager/workmanager.dart';

import 'hub.dart';
import 'main.dart' show backgroundCheck;
import 'screens.dart' show RequestScreen;

/// Lets a notification tap open a screen from outside the widget tree.
final navigator = GlobalKey<NavigatorState>();

/// Wake-ups while the app is closed, through UnifiedPush (the ntfy app, or any other
/// distributor). The PC posts the same sealed envelope it puts on the relay; we decrypt
/// it here and show an ordinary notification. Android only for now.
/// ponytail: Firebase (embedded FCM distributor) for phones without a distributor app.
class Push {
  static final _notes = FlutterLocalNotificationsPlugin();
  static bool _ready = false;

  /// Whether a distributor app is installed. False -> the app shows how to get one.
  static bool available = false;

  static Future<void> start(Hub hub) async {
    if (!Platform.isAndroid || _ready) return;
    _ready = true;
    await schedule();
    await _notes.initialize(
      settings: const InitializationSettings(android: AndroidInitializationSettings('@drawable/ic_stat_hearth')),
      onDidReceiveNotificationResponse: (r) => _open(hub, r.payload),
    );
    // Tapped while Hearth wasn't running: the tap started the app.
    final launch = await _notes.getNotificationAppLaunchDetails();
    if (launch?.didNotificationLaunchApp ?? false) _open(hub, launch!.notificationResponse?.payload);
    // Listen before UnifiedPush starts: a wake-up can deliver its message during startup.
    hub.addListener(() {
      _register(hub); // pairing done later -> register then
      _announce(hub);
    });
    await UnifiedPush.initialize(
      onNewEndpoint: (e, _) => hub.setPush(e.url),
      onUnregistered: (_) => hub.setPush(null),
      onMessage: (m, _) => _onMessage(hub, m.content),
    );
    available = await UnifiedPush.tryUseCurrentOrDefaultDistributor();
    hub.touch(); // the Today hint depends on this
    await _announce(hub); // anything that arrived before we were listening
    await _register(hub);
  }

  /// Hourly check, in case a wake-up never came (ntfy not running, phone offline).
  /// Android may run it later to save battery; Today also has pull-to-refresh.
  static Future<void> schedule() async {
    await Workmanager().initialize(backgroundCheck);
    await Workmanager().registerPeriodicTask(
      'hearth-check',
      'check',
      frequency: const Duration(hours: 1),
      existingWorkPolicy: ExistingPeriodicWorkPolicy.keep,
      constraints: Constraints(networkType: NetworkType.connected),
    );
  }

  /// WorkManager's side: fetch what the PC sent since the app last looked and
  /// announce new requests. Needs no answer from the PC, so it works while it's off.
  static void runChecks(Future<void> Function() init) => Workmanager().executeTask((_, _) async {
    try {
      await init();
      final hub = await Hub.open();
      if (!hub.paired) return true;
      await _notes.initialize(
        settings: const InitializationSettings(android: AndroidInitializationSettings('@drawable/ic_stat_hearth')),
      );
      _ready = true;
      final news = await hub.poll();
      for (final m in news) {
        hub.deliver(m);
      }
      await _announce(hub);
      debugPrint('hearth: background check: ${news.length} new');
    } catch (e) {
      debugPrint('hearth: background check failed: $e');
    }
    return true; // a failed check just waits for the next hour
  });

  /// Installed distributors (Android package names) and the one in use.
  static Future<List<String>> distributors() async => Platform.isAndroid ? await UnifiedPush.getDistributors() : [];
  static Future<String?> current() async => Platform.isAndroid ? await UnifiedPush.getDistributor() : null;

  /// Switch distributor: unregister from the old one, register with the new; the
  /// new endpoint reaches the PC through onNewEndpoint.
  static Future<void> use(Hub hub, String distributor) async {
    await UnifiedPush.unregister();
    await UnifiedPush.saveDistributor(distributor);
    available = true;
    _registered = false;
    await _register(hub);
  }

  /// Friendly name for a distributor package.
  static String label(String pkg) => switch (pkg) {
    'io.heckel.ntfy' => 'ntfy',
    _ when pkg.contains('murena') || pkg.startsWith('foundation.e') => 'Murena (/e/OS)',
    _ when pkg.contains('microg') || pkg == 'com.google.android.gms' => 'microG',
    _ => pkg,
  };

  static bool _registered = false;

  static Future<void> _register(Hub hub) async {
    if (!available || !hub.paired || _registered) return;
    _registered = true;
    await UnifiedPush.register();
    await _notes
        .resolvePlatformSpecificImplementation<AndroidFlutterLocalNotificationsPlugin>()
        ?.requestNotificationsPermission();
  }

  static Future<void> _onMessage(Hub hub, List<int> content) async {
    try {
      final msg = await hub.openPush(content);
      if (msg != null) hub.deliver(msg);
    } catch (e) {
      // Normal when the relay listener got the same message first.
      debugPrint('push message dropped: $e');
    }
  }

  /// Open the answer screen for request `id`. After a cold start the request may not
  /// be here yet (the PC resends unanswered ones when asked for a status), so wait for
  /// it a little.
  static Future<void> _open(Hub hub, String? id) async {
    debugPrint('hearth: notification tapped ($id)');
    if (id == null) return;
    for (var i = 0; i < 40; i++) {
      final req = hub.requests.where((r) => r.id == id).firstOrNull;
      final nav = navigator.currentState;
      if (req != null && nav != null) {
        nav.push(MaterialPageRoute(builder: (_) => RequestScreen(req: req)));
        return;
      }
      await Future<void>.delayed(const Duration(milliseconds: 250));
    }
    debugPrint('hearth: request $id not found after 10 s');
    // Answered elsewhere or gone: Today shows what's left.
  }

  /// Take an answered request's notification away.
  static Future<void> dismiss(String id) async {
    if (Platform.isAndroid && _ready) await _notes.cancel(id: id.hashCode);
  }

  /// Requests already announced, so each shows once whichever path brought it.
  static final Set<String> _shown = {};

  /// A new request while Hearth isn't on screen -> notification. Runs on every hub
  /// change, so it doesn't matter whether the push or the live relay delivered it.
  static Future<void> _announce(Hub hub) async {
    final inFront = WidgetsBinding.instance.lifecycleState == AppLifecycleState.resumed;
    for (final r in hub.requests) {
      if (!_shown.add(r.id) || inFront) continue;
      await _notes.show(
        id: r.id.hashCode,
        payload: r.id,
        title: '${r.child.name} wants ${r.minutes} more minutes',
        body: r.reason.isEmpty ? 'Tap to answer.' : '“${r.reason}”',
        notificationDetails: const NotificationDetails(
          android: AndroidNotificationDetails(
            'requests',
            'Time requests',
            channelDescription: 'A child asks for more screen time',
            importance: Importance.high,
            priority: Priority.high,
            icon: 'ic_stat_hearth',
            color: Color(0xFFE8622C),
          ),
        ),
      );
    }
  }
}

import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:unifiedpush/unifiedpush.dart';

import 'hub.dart';
import 'src/rust/api.dart' as rust;

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
    await _notes.initialize(
      settings: const InitializationSettings(android: AndroidInitializationSettings('@mipmap/ic_launcher')),
    );
    await UnifiedPush.initialize(
      onNewEndpoint: (e, _) => hub.setPush(e.url),
      onUnregistered: (_) => hub.setPush(null),
      onMessage: (m, _) => _onMessage(hub, m.content),
    );
    available = await UnifiedPush.tryUseCurrentOrDefaultDistributor();
    await _register(hub);
    hub.addListener(() => _register(hub)); // pairing done later -> register then
  }

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
    final rust.Msg? msg;
    try {
      msg = await hub.openPush(content);
    } catch (e) {
      debugPrint('push message dropped: $e'); // replayed (the live listener had it) or not ours
      return;
    }
    if (msg == null) return;
    hub.deliver(msg);
    final inFront = WidgetsBinding.instance.lifecycleState == AppLifecycleState.resumed;
    if (msg case rust.Msg_Request(:final id, :final user, :final minutes, :final reason) when !inFront) {
      await _notes.show(
        id: id.hashCode,
        title: '${Hub.display(user)} wants $minutes more minutes',
        body: reason.isEmpty ? 'Tap to answer.' : '“$reason”',
        notificationDetails: const NotificationDetails(
          android: AndroidNotificationDetails(
            'requests',
            'Time requests',
            channelDescription: 'A child asks for more screen time',
            importance: Importance.high,
            priority: Priority.high,
          ),
        ),
      );
    }
  }
}

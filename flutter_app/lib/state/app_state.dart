import 'package:flutter/foundation.dart';
import '../ffi/core_bridge.dart';

class AppState extends ChangeNotifier {
  int _listenPort = 7777;
  bool _initialized = false;
  String? _error;

  int get listenPort => _listenPort;
  bool get initialized => _initialized;
  String? get error => _error;
  String get myAddress => '127.0.0.1:$_listenPort';

  Future<void> initialize(int port) async {
    _listenPort = port;
    _error = null;
    try {
      CoreBridge.instance.initialize(port);
      await CoreBridge.instance.loadDefaultPlugins();
      _initialized = true;
    } catch (e) {
      _error = e.toString();
    }
    notifyListeners();
  }
}

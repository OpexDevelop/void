import 'package:flutter/foundation.dart';
import '../ffi/core_bridge.dart';

class AppState extends ChangeNotifier {
  String _myAddress = '';
  bool _initialized = false;
  String? _error;

  String get myAddress => _myAddress;
  bool get initialized => _initialized;
  String? get error => _error;

  Future<void> initialize({required String address}) async {
    _myAddress = address;
    _error = null;
    notifyListeners();

    try {
      // Всё тяжёлое в compute — не блокирует UI
      final error = await compute(_initInBackground, address);
      if (error != null) {
        _error = error;
      } else {
        _initialized = true;
      }
    } catch (e) {
      _error = e.toString();
    }
    notifyListeners();
  }
}

// Запускается в отдельном isolate
Future<String?> _initInBackground(String address) async {
  try {
    CoreBridge.instance.initialize();
    await CoreBridge.instance.loadDefaultPlugins();
    final ok = await CoreBridge.instance.configureTransport(
      myAddress: address,
    );
    if (!ok) return 'Failed to configure transport';
    return null;
  } catch (e) {
    return e.toString();
  }
}

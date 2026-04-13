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

    try {
      CoreBridge.instance.initialize();
      await CoreBridge.instance.loadDefaultPlugins();

      // Ядро говорит плагинам "вот наш адрес" — что с ним делать решает плагин
      final ok = await CoreBridge.instance.configureTransport(
        myAddress: address,
      );

      if (!ok) {
        _error = 'Failed to configure transport plugin';
        notifyListeners();
        return;
      }

      _initialized = true;
    } catch (e) {
      _error = e.toString();
    }
    notifyListeners();
  }
}

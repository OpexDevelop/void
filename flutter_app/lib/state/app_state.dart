import 'package:flutter/foundation.dart';
import '../ffi/core_bridge.dart';

class AppState extends ChangeNotifier {
  String myAddress = '';
  bool initialized = false;

  // Лог для отображения на экране при старте
  final List<String> log = [];

  void _log(String msg) {
    debugPrint('[AppState] $msg');
    log.add(msg);
    notifyListeners();
  }

  Future<void> initialize(String address) async {
    myAddress = address;
    initialized = false;
    log.clear();
    notifyListeners();

    await Future.delayed(const Duration(milliseconds: 30));

    _log('Starting core...');
    CoreBridge.instance.start();

    _log('Loading plugins...');
    final errors = await CoreBridge.instance.loadDefaultPlugins();

    for (final e in errors.entries) {
      _log('⚠ ${e.key}: ${e.value}');
    }

    _log('Configuring transport ($address)...');
    final transportErr = CoreBridge.instance.configureTransport(address);
    if (transportErr != null) {
      _log('⚠ Transport: $transportErr (offline mode)');
    } else {
      _log('Transport OK');
    }

    initialized = true;
    _log('Ready!');
    notifyListeners();
  }
}

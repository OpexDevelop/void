import 'package:flutter/foundation.dart';
import '../ffi/core_bridge.dart';

class AppState extends ChangeNotifier {
  String _myAddress = '';
  bool _initialized = false;
  bool _loading = false;
  String? _error;
  List<String> _warnings = [];

  String get myAddress => _myAddress;
  bool get initialized => _initialized;
  bool get loading => _loading;
  String? get error => _error;
  List<String> get warnings => List.unmodifiable(_warnings);

  Future<void> initialize({required String address}) async {
    _myAddress = address;
    _error = null;
    _warnings = [];
    _loading = true;
    notifyListeners();

    // Маленькая задержка чтобы UI успел перерисоваться
    await Future.delayed(const Duration(milliseconds: 50));

    try {
      CoreBridge.instance.initialize();
      await Future.delayed(const Duration(milliseconds: 10));

      // Загружаем плагины — ошибки не фатальны, собираем в warnings
      final pluginErrors = await CoreBridge.instance.loadDefaultPlugins();

      if (pluginErrors.isNotEmpty) {
        for (final e in pluginErrors.entries) {
          _warnings.add('Plugin ${e.key}: ${e.value}');
          debugPrint('[AppState] plugin warning: ${e.key}: ${e.value}');
        }
      }

      await Future.delayed(const Duration(milliseconds: 10));

      // configure transport — тоже не фатально
      final transportResult =
          await CoreBridge.instance.configureTransport(myAddress: address);

      if (!transportResult.ok) {
        _warnings.add(
            'Transport not configured: ${transportResult.warning ?? transportResult.error ?? "unknown"}');
        debugPrint('[AppState] transport warning: ${transportResult.warning}');
      }

      // Всегда помечаем как initialized — пусть работает в offline если нет транспорта
      _initialized = true;
    } catch (e, stack) {
      debugPrint('[AppState] fatal error: $e\n$stack');
      _error = '$e';
    }

    _loading = false;
    notifyListeners();
  }
}

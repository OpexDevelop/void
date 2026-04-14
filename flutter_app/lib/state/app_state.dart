import 'package:flutter/foundation.dart';
import '../ffi/core_bridge.dart';

class AppState extends ChangeNotifier {
  String _myAddress = '';
  bool _initialized = false;
  bool _loading = false;
  String? _error;

  String get myAddress => _myAddress;
  bool get initialized => _initialized;
  bool get loading => _loading;
  String? get error => _error;

  Future<void> initialize({required String address}) async {
    _myAddress = address;
    _error = null;
    _loading = true;
    notifyListeners();

    await Future.delayed(const Duration(milliseconds: 50));

    try {
      CoreBridge.instance.initialize();
      await Future.delayed(const Duration(milliseconds: 10));

      // Показываем каждую ошибку загрузки плагина
      final pluginErrors = await CoreBridge.instance.loadDefaultPlugins();

      if (pluginErrors.isNotEmpty) {
        _error = pluginErrors.entries
            .map((e) => '${e.key}:\n${e.value}')
            .join('\n\n');
        _loading = false;
        notifyListeners();
        return;
      }

      await Future.delayed(const Duration(milliseconds: 10));

      final ok = await CoreBridge.instance.configureTransport(
        myAddress: address,
      );

      if (!ok) {
        // Показать список загруженных плагинов для диагностики
        final loaded = CoreBridge.instance.listPlugins();
        final loadedStr = loaded.isEmpty
            ? 'none'
            : loaded.map((p) => '${p.id} (${p.category})').join(', ');
        _error = 'Failed to configure transport.\n'
            'Loaded plugins: $loadedStr';
        _loading = false;
        notifyListeners();
        return;
      }

      _initialized = true;
    } catch (e, stack) {
      _error = '$e\n\n$stack';
    }

    _loading = false;
    notifyListeners();
  }
}

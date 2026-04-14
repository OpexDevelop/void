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
    await initializeWithLog(address: address, onLog: (_) {});
  }

  Future<void> initializeWithLog({
    required String address,
    required void Function(String) onLog,
  }) async {
    _myAddress = address;
    _error = null;
    _loading = true;
    notifyListeners();

    await Future.delayed(const Duration(milliseconds: 30));

    try {
      onLog('coreInit...');
      CoreBridge.instance.initialize();
      onLog('coreInit OK');

      await Future.delayed(const Duration(milliseconds: 20));

      final plugins = [
        (
          'storage_memory',
          'assets/plugins/storage_memory.wasm',
          'assets/plugins/storage_memory.manifest.toml',
        ),
        (
          'crypto_aes',
          'assets/plugins/crypto_aes.wasm',
          'assets/plugins/crypto_aes.manifest.toml',
        ),
        (
          'transport_ntfy',
          'assets/plugins/transport_ntfy.wasm',
          'assets/plugins/transport_ntfy.manifest.toml',
        ),
      ];

      for (final (name, wasmPath, manifestPath) in plugins) {
        onLog('loading $name...');
        try {
          await CoreBridge.instance.loadOnePlugin(
            name: name,
            wasmPath: wasmPath,
            manifestPath: manifestPath,
          );
          onLog('$name OK');
        } catch (e) {
          onLog('$name FAILED: $e');
          // Не останавливаемся — пробуем остальные
        }
      }

      onLog('configure transport: $address...');
      try {
        final result = await CoreBridge.instance.configureTransport(
          myAddress: address,
        );
        onLog('transport: $result');
      } catch (e) {
        onLog('transport configure failed: $e (offline mode)');
      }

      _initialized = true;
      onLog('initialized!');
    } catch (e, st) {
      _error = '$e\n$st';
      onLog('FATAL: $e');
    }

    _loading = false;
    notifyListeners();
  }
}

import 'dart:async';
import 'dart:convert';
import 'dart:isolate';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'core_ffi.dart';
import '../models/message.dart';
import '../models/plugin_info.dart';

class CoreBridge {
  CoreBridge._();
  static final CoreBridge instance = CoreBridge._();

  final _messageController = StreamController<Message>.broadcast();
  Stream<Message> get messageStream => _messageController.stream;

  Timer? _eventPollTimer;
  Timer? _transportPollTimer;
  int _lastSeenTimestamp = 0;

  void initialize() {
    coreInit();

    _eventPollTimer?.cancel();
    _eventPollTimer = Timer.periodic(const Duration(milliseconds: 150), (_) {
      _drainEvents();
    });

    _transportPollTimer?.cancel();
    _transportPollTimer = Timer.periodic(const Duration(seconds: 3), (_) {
      _pollTransport();
    });
  }

  void _drainEvents() {
    while (true) {
      final json = corePollEvent();
      if (json == null) break;
      try {
        final event = jsonDecode(json) as Map<String, dynamic>;
        if (event['kind'] == 'message_received') {
          final p = event['payload'] as Map<String, dynamic>;
          final ts = (p['timestamp'] as num?)?.toInt() ?? 0;
          _messageController.add(Message(
            from: p['from'] as String,
            to: 'me',
            text: p['text'] as String,
            timestamp: DateTime.fromMillisecondsSinceEpoch(ts * 1000),
          ));
          if (ts > _lastSeenTimestamp) {
            _lastSeenTimestamp = ts;
          }
        }
      } catch (_) {}
    }
  }

  void _pollTransport() {
    corePollTransport(_lastSeenTimestamp.toString());
  }

  Future<bool> configureTransport({required String myAddress}) async {
    final result = jsonDecode(coreConfigureTransport(myAddress));
    return result['ok'] == true;
  }

  /// Загружает один плагин с таймаутом чтобы не висеть вечно
  Future<String?> _loadOnePlugin(
    String name,
    String wasmPath,
    String manifestPath,
  ) async {
    try {
      final wasmData = await rootBundle.load(wasmPath);
      final manifestRaw = await rootBundle.loadString(manifestPath);
      final manifest = manifestRaw
          .trimLeft()
          .replaceAll('\r\n', '\n')
          .replaceAll('\r', '\n');

      final wasmBytes = wasmData.buffer.asUint8List().toList();

      debugPrint('[$name] wasm size: ${wasmBytes.length} bytes');
      debugPrint('[$name] manifest: ${manifest.substring(0, manifest.length.clamp(0, 80))}');

      // Даём UI вздохнуть перед тяжёлым FFI вызовом
      await Future.delayed(const Duration(milliseconds: 16));

      final response = coreLoadPlugin(wasmBytes, manifest);
      final json = jsonDecode(response);

      if (json['ok'] == true) {
        debugPrint('[$name] loaded OK');
        return null; // null = успех
      } else {
        final err = json['error']?.toString() ?? 'unknown error';
        debugPrint('[$name] FAILED: $err');
        return err;
      }
    } catch (e) {
      debugPrint('[$name] EXCEPTION: $e');
      return e.toString();
    }
  }

  Future<Map<String, String>> loadDefaultPlugins() async {
    final errors = <String, String>{};

    final defaults = [
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

    for (final (name, wasmPath, manifestPath) in defaults) {
      // Таймаут 30 секунд на каждый плагин
      final error = await _loadOnePlugin(name, wasmPath, manifestPath)
          .timeout(
        const Duration(seconds: 30),
        onTimeout: () => 'timeout after 30s',
      );

      if (error != null) {
        errors[name] = error;
      }

      // Пауза между плагинами чтобы UI не замерзал
      await Future.delayed(const Duration(milliseconds: 32));
    }

    return errors;
  }

  Future<PluginInfo?> loadPlugin(List<int> wasmBytes, String manifest) async {
    await Future.delayed(const Duration(milliseconds: 16));
    final response = coreLoadPlugin(wasmBytes, manifest);
    final json = jsonDecode(response);
    if (json['ok'] == true) {
      return PluginInfo.fromJson(json['plugin'] as Map<String, dynamic>);
    } else {
      throw Exception(json['error'] ?? 'Unknown Rust error');
    }
  }

  List<PluginInfo> listPlugins() {
    final list = jsonDecode(coreListPlugins()) as List;
    return list
        .map((e) => PluginInfo.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  void unloadPlugin(String id) => coreUnloadPlugin(id);

  Future<bool> sendMessage(String to, String text) async {
    final result =
        jsonDecode(coreSendMessage(to, text)) as Map<String, dynamic>;
    return result['ok'] == true;
  }

  List<Message> getMessages(String contact) {
    final list = jsonDecode(coreGetMessages(contact)) as List;
    return list
        .map((e) => Message.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  void dispose() {
    _eventPollTimer?.cancel();
    _transportPollTimer?.cancel();
    _messageController.close();
  }
}

import 'dart:async';
import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'core_ffi.dart';
import '../models/message.dart';
import '../models/plugin_info.dart';

class TransportConfigResult {
  final bool ok;
  final String? warning;
  final String? error;

  TransportConfigResult({required this.ok, this.warning, this.error});

  factory TransportConfigResult.fromJson(Map<String, dynamic> json) {
    return TransportConfigResult(
      ok: json['ok'] as bool? ?? false,
      warning: json['warning'] as String?,
      error: json['error'] as String?,
    );
  }
}

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
    _eventPollTimer =
        Timer.periodic(const Duration(milliseconds: 200), (_) {
      _drainEvents();
    });

    _transportPollTimer?.cancel();
    _transportPollTimer =
        Timer.periodic(const Duration(seconds: 5), (_) {
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
      } catch (e) {
        debugPrint('[CoreBridge] event parse error: $e');
      }
    }
  }

  void _pollTransport() {
    try {
      corePollTransport(_lastSeenTimestamp.toString());
    } catch (e) {
      debugPrint('[CoreBridge] pollTransport error: $e');
    }
  }

  Future<TransportConfigResult> configureTransport(
      {required String myAddress}) async {
    try {
      final raw = coreConfigureTransport(myAddress);
      final json = jsonDecode(raw) as Map<String, dynamic>;
      return TransportConfigResult.fromJson(json);
    } catch (e) {
      return TransportConfigResult(ok: false, error: e.toString());
    }
  }

  /// Загружает встроенные плагины из assets.
  /// Возвращает Map<pluginName, errorMessage> — пустой если всё ок.
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
      try {
        final wasmData = await rootBundle.load(wasmPath);
        final manifestRaw = await rootBundle.loadString(manifestPath);
        final manifest = manifestRaw
            .trimLeft()
            .replaceAll('\r\n', '\n')
            .replaceAll('\r', '\n');

        debugPrint('[CoreBridge] Loading plugin $name...');

        await loadPlugin(
          wasmData.buffer.asUint8List().toList(),
          manifest,
        );

        debugPrint('[CoreBridge] Plugin $name loaded OK');
      } on PluginAlreadyLoadedException {
        // Уже загружен — не ошибка
        debugPrint('[CoreBridge] Plugin $name already loaded, skipping');
      } catch (e) {
        debugPrint('[CoreBridge] Plugin $name FAILED: $e');
        errors[name] = e.toString();
      }
    }

    return errors;
  }

  Future<PluginInfo?> loadPlugin(List<int> wasmBytes, String manifest) async {
    final response = coreLoadPlugin(wasmBytes, manifest);
    debugPrint('[CoreBridge] loadPlugin raw response: $response');

    final json = jsonDecode(response) as Map<String, dynamic>;
    if (json['ok'] == true) {
      return PluginInfo.fromJson(json['plugin'] as Map<String, dynamic>);
    }

    final errorMsg = json['error'] as String? ?? 'Unknown Rust error';

    // Проверяем — это "already loaded"?
    if (errorMsg.contains('already loaded')) {
      throw PluginAlreadyLoadedException(errorMsg);
    }

    throw Exception(errorMsg);
  }

  List<PluginInfo> listPlugins() {
    final list = jsonDecode(coreListPlugins()) as List;
    return list
        .map((e) => PluginInfo.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  void unloadPlugin(String id) => coreUnloadPlugin(id);

  Future<bool> sendMessage(String to, String text) async {
    try {
      final result =
          jsonDecode(coreSendMessage(to, text)) as Map<String, dynamic>;
      return result['ok'] == true;
    } catch (e) {
      debugPrint('[CoreBridge] sendMessage error: $e');
      return false;
    }
  }

  List<Message> getMessages(String contact) {
    try {
      final list = jsonDecode(coreGetMessages(contact)) as List;
      return list
          .map((e) => Message.fromJson(e as Map<String, dynamic>))
          .toList();
    } catch (e) {
      debugPrint('[CoreBridge] getMessages error: $e');
      return [];
    }
  }

  void dispose() {
    _eventPollTimer?.cancel();
    _transportPollTimer?.cancel();
    _messageController.close();
  }
}

class PluginAlreadyLoadedException implements Exception {
  final String message;
  PluginAlreadyLoadedException(this.message);

  @override
  String toString() => message;
}

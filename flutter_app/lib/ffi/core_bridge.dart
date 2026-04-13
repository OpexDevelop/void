import 'dart:async';
import 'dart:convert';
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

  /// Dart знает только: "у нас есть адрес, передай его транспорту"
  /// Что такое адрес для конкретного плагина — не его дело
  Future<bool> configureTransport({required String myAddress}) async {
    final result = jsonDecode(coreConfigureTransport(myAddress));
    return result['ok'] == true;
  }

  Future<void> loadDefaultPlugins() async {
    final defaults = [
      (
        'storage_memory',
        'assets/plugins/storage_memory.wasm',
        'assets/plugins/storage_memory.manifest.toml'
      ),
      (
        'crypto_aes',
        'assets/plugins/crypto_aes.wasm',
        'assets/plugins/crypto_aes.manifest.toml'
      ),
      (
        'transport_ntfy',
        'assets/plugins/transport_ntfy.wasm',
        'assets/plugins/transport_ntfy.manifest.toml'
      ),
    ];

    for (final (name, wasmPath, manifestPath) in defaults) {
      try {
        final wasmData = await rootBundle.load(wasmPath);
        final manifestStr = await rootBundle.loadString(manifestPath);
        await loadPlugin(wasmData.buffer.asUint8List().toList(), manifestStr);
      } catch (e) {
        print('Default plugin $name not found in assets, skipping: $e');
      }
    }
  }

  Future<PluginInfo?> loadPlugin(List<int> wasmBytes, String manifest) async {
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

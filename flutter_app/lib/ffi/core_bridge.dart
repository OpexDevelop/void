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
  Timer? _pollTimer;

  void initialize(int port) {
    coreInit(port);
    _pollTimer?.cancel();
    _pollTimer = Timer.periodic(const Duration(milliseconds: 150), (_) => _drainEvents());
  }

  void _drainEvents() {
    while (true) {
      final json = corePollEvent();
      if (json == null) break;
      try {
        final event = jsonDecode(json) as Map<String, dynamic>;
        if (event['kind'] == 'message_received') {
          final p = event['payload'] as Map<String, dynamic>;
          _messageController.add(Message(
            from: p['from'] as String,
            to: 'me',
            text: p['text'] as String,
            timestamp: DateTime.fromMillisecondsSinceEpoch(((p['timestamp'] as num?) ?? 0).toInt() * 1000),
          ));
        }
      } catch (_) {}
    }
  }

  Future<void> loadDefaultPlugins() async {
    final defaults = [
      ('storage_memory', 'assets/plugins/storage_memory.wasm', 'assets/plugins/storage_memory.manifest.toml'),
      ('crypto_aes', 'assets/plugins/crypto_aes.wasm', 'assets/plugins/crypto_aes.manifest.toml'),
    ];
    for (final (name, wasmPath, manifestPath) in defaults) {
      try {
        final wasmData = await rootBundle.load(wasmPath);
        final manifestStr = await rootBundle.loadString(manifestPath);
        await loadPlugin(wasmData.buffer.asUint8List().toList(), manifestStr);
      } catch (e) { print('Skipping $name: $e'); }
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
    return list.map((e) => PluginInfo.fromJson(e as Map<String, dynamic>)).toList();
  }

  void unloadPlugin(String id) => coreUnloadPlugin(id);

  Future<bool> sendMessage(String to, String text) async {
    final result = jsonDecode(coreSendMessage(to, text)) as Map<String, dynamic>;
    return result['ok'] == true;
  }

  List<Message> getMessages(String contact) {
    final list = jsonDecode(coreGetMessages(contact)) as List;
    return list.map((e) => Message.fromJson(e as Map<String, dynamic>)).toList();
  }
}

import 'dart:async';
import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'core_ffi.dart';
import '../models/message.dart';
import '../models/plugin_info.dart';

class CoreBridge {
  CoreBridge._();
  static final CoreBridge instance = CoreBridge._();

  final _messages = StreamController<Message>.broadcast();
  Stream<Message> get messageStream => _messages.stream;

  Timer? _eventTimer;
  Timer? _transportTimer;
  int _lastTs = 0;

  // ── init ────────────────────────────────────────────────────────────

  void start() {
    coreInit();
    _eventTimer?.cancel();
    _transportTimer?.cancel();
    _eventTimer = Timer.periodic(const Duration(milliseconds: 200), (_) => _drainEvents());
    _transportTimer = Timer.periodic(const Duration(seconds: 5), (_) => _poll());
  }

  void _drainEvents() {
    for (;;) {
      final raw = corePollEvent();
      if (raw == null) break;
      try {
        final e = jsonDecode(raw) as Map<String, dynamic>;
        if (e['kind'] == 'message_received') {
          final p = e['payload'] as Map<String, dynamic>;
          final ts = (p['timestamp'] as num?)?.toInt() ?? 0;
          _messages.add(Message(
            from: p['from'] as String,
            to: 'me',
            text: p['text'] as String,
            timestamp: DateTime.fromMillisecondsSinceEpoch(ts * 1000),
          ));
          if (ts > _lastTs) _lastTs = ts;
        }
      } catch (_) {}
    }
  }

  void _poll() {
    try {
      corePollTransport(_lastTs.toString());
    } catch (_) {}
  }

  // ── plugins ─────────────────────────────────────────────────────────

  /// Возвращает null если всё ок, строку с ошибкой если что-то пошло не так.
  Future<String?> loadAssetPlugin(String name) async {
    try {
      final wasmData = await rootBundle.load('assets/plugins/$name.wasm');
      final toml = await rootBundle.loadString('assets/plugins/$name.manifest.toml');
      final manifest = toml.trimLeft().replaceAll('\r\n', '\n').replaceAll('\r', '\n');
      final bytes = wasmData.buffer.asUint8List().toList();

      final raw = coreLoadPlugin(bytes, manifest);
      debugPrint('[CoreBridge] $name: $raw');

      final json = jsonDecode(raw) as Map<String, dynamic>;
      if (json['ok'] == true) return null;

      final err = json['error'] as String? ?? 'unknown';
      // "already loaded" не ошибка
      if (err.contains('already loaded')) return null;
      return err;
    } catch (e) {
      return e.toString();
    }
  }

  Future<Map<String, String>> loadDefaultPlugins() async {
    final errors = <String, String>{};
    for (final name in ['storage_memory', 'crypto_aes', 'transport_ntfy']) {
      final err = await loadAssetPlugin(name);
      if (err != null) errors[name] = err;
    }
    return errors;
  }

  /// Возвращает null если ок, строку с ошибкой если нет.
  String? configureTransport(String address) {
    try {
      final raw = coreConfigureTransport(address);
      debugPrint('[CoreBridge] configureTransport: $raw');
      final json = jsonDecode(raw) as Map<String, dynamic>;
      if (json['ok'] == true) return null;
      return json['error'] as String?;
    } catch (e) {
      return e.toString();
    }
  }

  Future<PluginInfo> installPlugin(List<int> bytes, String manifest) async {
    final raw = coreLoadPlugin(bytes, manifest);
    final json = jsonDecode(raw) as Map<String, dynamic>;
    if (json['ok'] == true) {
      return PluginInfo.fromJson(json['plugin'] as Map<String, dynamic>);
    }
    throw Exception(json['error'] ?? 'load failed');
  }

  List<PluginInfo> listPlugins() {
    final list = jsonDecode(coreListPlugins()) as List;
    return list.map((e) => PluginInfo.fromJson(e as Map<String, dynamic>)).toList();
  }

  void removePlugin(String id) => coreUnloadPlugin(id);

  // ── messaging ────────────────────────────────────────────────────────

  bool sendMessage(String to, String text) {
    try {
      final json = jsonDecode(coreSendMessage(to, text)) as Map<String, dynamic>;
      return json['ok'] == true;
    } catch (_) {
      return false;
    }
  }

  List<Message> getMessages(String contact) {
    try {
      final list = jsonDecode(coreGetMessages(contact)) as List;
      return list.map((e) => Message.fromJson(e as Map<String, dynamic>)).toList();
    } catch (_) {
      return [];
    }
  }
}

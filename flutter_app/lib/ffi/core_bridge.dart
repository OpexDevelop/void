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
          if (ts > _lastSeenTimestamp) _lastSeenTimestamp = ts;
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

  /// Загружает один плагин из assets по имени
  Future<PluginInfo> loadOnePlugin({
    required String name,
    required String wasmPath,
    required String manifestPath,
  }) async {
    final wasmData = await rootBundle.load(wasmPath);
    final manifestRaw = await rootBundle.loadString(manifestPath);
    final manifest = manifestRaw
        .trimLeft()
        .replaceAll('\r\n', '\n')
        .replaceAll('\r', '\n');

    return await loadPlugin(
      wasmData.buffer.asUint8List().toList(),
      manifest,
    );
  }

  Future<PluginInfo> loadPlugin(List<int> wasmBytes, String manifest) async {
    final response = coreLoadPlugin(wasmBytes, manifest);
    debugPrint('[CoreBridge] loadPlugin response: $response');

    final json = jsonDecode(response) as Map<String, dynamic>;
    if (json['ok'] == true) {
      return PluginInfo.fromJson(json['plugin'] as Map<String, dynamic>);
    }

    final err = json['error'] as String? ?? 'unknown error from Rust';
    throw Exception(err);
  }

  Future<String> configureTransport({required String myAddress}) async {
    final raw = coreConfigureTransport(myAddress);
    debugPrint('[CoreBridge] configureTransport response: $raw');
    final json = jsonDecode(raw) as Map<String, dynamic>;
    if (json['ok'] == true) {
      return json['warning'] as String? ?? 'ok';
    }
    throw Exception(json['error'] ?? 'configure transport failed');
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

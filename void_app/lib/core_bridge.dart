import 'dart:convert';
import 'dart:ffi';
import 'package:ffi/ffi.dart';

typedef _InitC = Int32 Function(Int32 port, Pointer<Uint8> key);
typedef _InitDart = int Function(int port, Pointer<Uint8> key);

typedef _SendC = Int32 Function(Pointer<Utf8> chatId, Pointer<Utf8> text, Pointer<Utf8> addr);
typedef _SendDart = int Function(Pointer<Utf8> chatId, Pointer<Utf8> text, Pointer<Utf8> addr);

typedef _PollC = Pointer<Utf8> Function();
typedef _PollDart = Pointer<Utf8> Function();

typedef _HistoryC = Pointer<Utf8> Function(Pointer<Utf8> chatId);
typedef _HistoryDart = Pointer<Utf8> Function(Pointer<Utf8> chatId);

typedef _FreeC = Void Function(Pointer<Utf8> ptr);
typedef _FreeDart = void Function(Pointer<Utf8> ptr);

class VoidCore {
  late final _SendDart _send;
  late final _PollDart _poll;
  late final _HistoryDart _history;
  late final _FreeDart _free;

  static VoidCore? _instance;

  VoidCore._();

  static VoidCore init({required int port, required List<int> key}) {
    if (_instance != null) return _instance!;

    final lib = DynamicLibrary.open('libvoid_core.so');
    final core = VoidCore._();

    final initFn = lib.lookupFunction<_InitC, _InitDart>('void_init');
    core._send = lib.lookupFunction<_SendC, _SendDart>('void_send');
    core._poll = lib.lookupFunction<_PollC, _PollDart>('void_poll');
    core._history = lib.lookupFunction<_HistoryC, _HistoryDart>('void_history');
    core._free = lib.lookupFunction<_FreeC, _FreeDart>('void_free');

    final keyPtr = calloc<Uint8>(32);
    for (var i = 0; i < 32; i++) {
      keyPtr[i] = i < key.length ? key[i] : 0;
    }
    initFn(port, keyPtr);
    calloc.free(keyPtr);

    _instance = core;
    return core;
  }

  void sendMessage(String chatId, String text, String peerAddr) {
    final c1 = chatId.toNativeUtf8();
    final c2 = text.toNativeUtf8();
    final c3 = peerAddr.toNativeUtf8();
    _send(c1, c2, c3);
    calloc.free(c1);
    calloc.free(c2);
    calloc.free(c3);
  }

  Map<String, dynamic>? poll() {
    final ptr = _poll();
    if (ptr == nullptr) return null;
    final json = ptr.toDartString();
    _free(ptr);
    return Map<String, dynamic>.from(
      const JsonDecoder().convert(json) as Map,
    );
  }
}

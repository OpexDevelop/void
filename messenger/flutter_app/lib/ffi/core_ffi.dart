import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';

DynamicLibrary _loadLib() {
  if (Platform.isAndroid) return DynamicLibrary.open('libmessenger_core.so');
  if (Platform.isIOS) return DynamicLibrary.process();
  if (Platform.isMacOS) return DynamicLibrary.open('libmessenger_core.dylib');
  if (Platform.isLinux) return DynamicLibrary.open('libmessenger_core.so');
  if (Platform.isWindows) return DynamicLibrary.open('messenger_core.dll');
  throw UnsupportedError('Unsupported platform: ${Platform.operatingSystem}');
}

final _lib = _loadLib();

final _init = _lib.lookupFunction<Void Function(Int32), void Function(int)>('messenger_init');
final _loadPlugin = _lib.lookupFunction<Pointer<Utf8> Function(Pointer<Uint8>, Int32, Pointer<Utf8>), Pointer<Utf8> Function(Pointer<Uint8>, int, Pointer<Utf8>)>('messenger_load_plugin');
final _listPlugins = _lib.lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>('messenger_list_plugins');
final _unloadPlugin = _lib.lookupFunction<Void Function(Pointer<Utf8>), void Function(Pointer<Utf8>)>('messenger_unload_plugin');
final _sendMessage = _lib.lookupFunction<Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>), Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>)>('messenger_send_message');
final _getMessages = _lib.lookupFunction<Pointer<Utf8> Function(Pointer<Utf8>), Pointer<Utf8> Function(Pointer<Utf8>)>('messenger_get_messages');
final _pollEvent = _lib.lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>('messenger_poll_event');
final _freeString = _lib.lookupFunction<Void Function(Pointer<Utf8>), void Function(Pointer<Utf8>)>('messenger_free_string');

String? _readAndFree(Pointer<Utf8> ptr) {
  if (ptr.address == 0) return null;
  final str = ptr.toDartString();
  _freeString(ptr);
  return str;
}

void coreInit(int port) => _init(port);

String coreLoadPlugin(List<int> wasmBytes, String manifest) {
  final wasmPtr = malloc.allocate<Uint8>(wasmBytes.length);
  for (int i = 0; i < wasmBytes.length; i++) {
    wasmPtr[i] = wasmBytes[i];
  }
  final manifestPtr = manifest.toNativeUtf8();
  final result = _loadPlugin(wasmPtr, wasmBytes.length, manifestPtr);
  malloc.free(wasmPtr);
  malloc.free(manifestPtr);
  return _readAndFree(result) ?? '{"ok":false,"error":"null result"}';
}

String coreListPlugins() => _readAndFree(_listPlugins()) ?? '[]';

void coreUnloadPlugin(String id) {
  final ptr = id.toNativeUtf8();
  _unloadPlugin(ptr);
  malloc.free(ptr);
}

String coreSendMessage(String to, String text) {
  final toPtr = to.toNativeUtf8();
  final textPtr = text.toNativeUtf8();
  final result = _sendMessage(toPtr, textPtr);
  malloc.free(toPtr);
  malloc.free(textPtr);
  return _readAndFree(result) ?? '{"ok":false}';
}

String coreGetMessages(String contact) {
  final ptr = contact.toNativeUtf8();
  final result = _getMessages(ptr);
  malloc.free(ptr);
  return _readAndFree(result) ?? '[]';
}

String? corePollEvent() => _readAndFree(_pollEvent());

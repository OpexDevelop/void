import 'package:flutter/foundation.dart';
import 'package:file_picker/file_picker.dart';
import '../ffi/core_bridge.dart';
import '../models/plugin_info.dart';

class PluginsState extends ChangeNotifier {
  List<PluginInfo> _plugins = [];
  bool _loading = false;
  String? _lastError;

  List<PluginInfo> get plugins => List.unmodifiable(_plugins);
  bool get loading => _loading;
  String? get lastError => _lastError;

  void refresh() {
    _plugins = CoreBridge.instance.listPlugins();
    notifyListeners();
  }

  Future<void> pickAndInstall() async {
    _lastError = null;
    notifyListeners();

    // Выбираем оба файла за один раз
    final result = await FilePicker.platform.pickFiles(
      type: FileType.any,
      withData: true,
      allowMultiple: true,
      dialogTitle: 'Select .wasm and .toml files',
    );

    if (result == null || result.files.isEmpty) return;

    // Находим wasm и toml среди выбранных
    PlatformFile? wasmFile;
    PlatformFile? tomlFile;

    for (final file in result.files) {
      final name = file.name.toLowerCase();
      if (name.endsWith('.wasm')) {
        wasmFile = file;
      } else if (name.endsWith('.toml')) {
        tomlFile = file;
      }
    }

    // Если не выбрали оба — просим по одному
    if (wasmFile == null) {
      _lastError = 'No .wasm file selected';
      notifyListeners();
      return;
    }

    if (tomlFile == null) {
      _lastError = 'No .toml manifest file selected';
      notifyListeners();
      return;
    }

    final wasmBytes = wasmFile.bytes;
    final tomlBytes = tomlFile.bytes;

    if (wasmBytes == null) {
      _lastError = 'Failed to read .wasm file';
      notifyListeners();
      return;
    }

    if (tomlBytes == null) {
      _lastError = 'Failed to read .toml file';
      notifyListeners();
      return;
    }

    _loading = true;
    notifyListeners();

    // Убираем BOM и нормализуем переносы строк
    final manifestStr = String.fromCharCodes(tomlBytes)
        .trimLeft()
        .replaceAll('\r\n', '\n')
        .replaceAll('\r', '\n');

    try {
      final info = await CoreBridge.instance.loadPlugin(
        wasmBytes.toList(),
        manifestStr,
      );

      if (info != null) {
        _plugins = CoreBridge.instance.listPlugins();
      }
    } catch (e) {
      _lastError = e.toString().replaceAll('Exception: ', '');
    } finally {
      _loading = false;
      notifyListeners();
    }
  }

  void unload(String id) {
    CoreBridge.instance.unloadPlugin(id);
    _plugins = CoreBridge.instance.listPlugins();
    notifyListeners();
  }
}

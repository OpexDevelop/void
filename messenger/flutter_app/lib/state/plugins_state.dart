import 'dart:io';
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

    // 1. Просим выбрать WASM
    final wasmResult = await FilePicker.platform.pickFiles(
      type: FileType.any,
      withData: true,
      dialogTitle: 'Select .wasm plugin file',
    );

    if (wasmResult == null || wasmResult.files.isEmpty) return;
    final wasmFile = wasmResult.files.first;

    if (!wasmFile.name.endsWith('.wasm')) {
      _lastError = 'Please select a .wasm file first';
      notifyListeners();
      return;
    }

    // 2. Просим выбрать TOML
    final tomlResult = await FilePicker.platform.pickFiles(
      type: FileType.any,
      withData: true,
      dialogTitle: 'Select manifest.toml file',
    );

    if (tomlResult == null || tomlResult.files.isEmpty) return;
    final tomlFile = tomlResult.files.first;

    if (!tomlFile.name.endsWith('.toml')) {
      _lastError = 'Please select a .toml manifest file';
      notifyListeners();
      return;
    }

    _loading = true;
    notifyListeners();

    // Читаем манифест
    final manifestStr = String.fromCharCodes(tomlFile.bytes!);

    try {
      // Загружаем в ядро
      final info = await CoreBridge.instance.loadPlugin(
        wasmFile.bytes!.toList(),
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

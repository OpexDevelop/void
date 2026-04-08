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

    final wasmResult = await FilePicker.platform.pickFiles(type: FileType.any, withData: true, dialogTitle: 'Select .wasm plugin');
    if (wasmResult == null || wasmResult.files.isEmpty) return;
    
    final tomlResult = await FilePicker.platform.pickFiles(type: FileType.any, withData: true, dialogTitle: 'Select manifest.toml');
    if (tomlResult == null || tomlResult.files.isEmpty) return;

    _loading = true;
    notifyListeners();

    try {
      final manifestStr = String.fromCharCodes(tomlResult.files.first.bytes!);
      await CoreBridge.instance.loadPlugin(wasmResult.files.first.bytes!.toList(), manifestStr);
      _plugins = CoreBridge.instance.listPlugins();
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

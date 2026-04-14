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
    try {
      _plugins = CoreBridge.instance.listPlugins();
    } catch (e) {
      debugPrint('[PluginsState] refresh error: $e');
      _plugins = [];
    }
    notifyListeners();
  }

  Future<void> pickAndInstall() async {
    _lastError = null;
    notifyListeners();

    final result = await FilePicker.platform.pickFiles(
      type: FileType.any,
      withData: true,
      allowMultiple: true,
      dialogTitle: 'Select .wasm and .toml files',
    );

    if (result == null || result.files.isEmpty) return;

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

    if (wasmBytes == null || tomlBytes == null) {
      _lastError = 'Failed to read files';
      notifyListeners();
      return;
    }

    _loading = true;
    notifyListeners();

    final manifestStr = String.fromCharCodes(tomlBytes)
        .trimLeft()
        .replaceAll('\r\n', '\n')
        .replaceAll('\r', '\n');

    try {
      await CoreBridge.instance.loadPlugin(
        wasmBytes.toList(),
        manifestStr,
      );
      _plugins = CoreBridge.instance.listPlugins();
    } on PluginAlreadyLoadedException catch (e) {
      _lastError = 'Plugin already loaded: $e';
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

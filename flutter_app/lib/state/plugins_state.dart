import 'package:flutter/foundation.dart';
import 'package:file_picker/file_picker.dart';
import '../ffi/core_bridge.dart';
import '../models/plugin_info.dart';

class PluginsState extends ChangeNotifier {
  List<PluginInfo> plugins = [];
  bool loading = false;
  String? error;

  void refresh() {
    plugins = CoreBridge.instance.listPlugins();
    notifyListeners();
  }

  Future<void> pickAndInstall() async {
    error = null;
    notifyListeners();

    final result = await FilePicker.platform.pickFiles(
      withData: true,
      allowMultiple: true,
      dialogTitle: 'Select .wasm and .toml',
    );
    if (result == null) return;

    PlatformFile? wasmFile;
    PlatformFile? tomlFile;
    for (final f in result.files) {
      if (f.name.endsWith('.wasm')) wasmFile = f;
      if (f.name.endsWith('.toml')) tomlFile = f;
    }

    if (wasmFile?.bytes == null) { error = 'No .wasm selected'; notifyListeners(); return; }
    if (tomlFile?.bytes == null) { error = 'No .toml selected'; notifyListeners(); return; }

    loading = true;
    notifyListeners();

    try {
      final manifest = String.fromCharCodes(tomlFile!.bytes!)
          .trimLeft()
          .replaceAll('\r\n', '\n')
          .replaceAll('\r', '\n');
      await CoreBridge.instance.installPlugin(wasmFile!.bytes!.toList(), manifest);
      plugins = CoreBridge.instance.listPlugins();
    } catch (e) {
      error = e.toString().replaceAll('Exception: ', '');
    }

    loading = false;
    notifyListeners();
  }

  void unload(String id) {
    CoreBridge.instance.removePlugin(id);
    plugins = CoreBridge.instance.listPlugins();
    notifyListeners();
  }
}

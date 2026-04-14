import 'package:flutter/foundation.dart';
import 'package:file_picker/file_picker.dart';
import '../src/rust/api.dart' as rust;
import '../models/plugin_info.dart';

class PluginsState extends ChangeNotifier {
  List<PluginInfo> plugins = [];
  bool loading = false;
  String? error;

  void refresh() {
    try {
      final list = rust.listPlugins();
      plugins = list.map((p) => PluginInfo(
        id: p.id,
        name: p.name,
        version: p.version,
        category: p.category,
        description: p.description,
        active: p.active,
        network: p.network,
        filesystem: p.filesystem,
      )).toList();
    } catch (e) {
      debugPrint('[Plugins] refresh error: $e');
    }
    notifyListeners();
  }

  Future<void> pickAndInstall() async {
    error = null;
    notifyListeners();

    final result = await FilePicker.platform.pickFiles(
      withData: true,
      allowMultiple: true,
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

      await rust.loadPlugin(
        wasm: wasmFile!.bytes!.toList(),
        manifest: manifest,
      );
      refresh();
    } catch (e) {
      error = e.toString().replaceAll('Exception: ', '');
    }

    loading = false;
    notifyListeners();
  }

  void unload(String id) {
    rust.unloadPlugin(id: id);
    refresh();
  }
}

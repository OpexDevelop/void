import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import '../src/rust/frb_generated.dart';
import '../src/rust/api.dart';

class AppState extends ChangeNotifier {
  String myAddress = '';
  bool initialized = false;
  final List<String> log = [];

  void _log(String msg) {
    debugPrint('[App] $msg');
    log.add(msg);
    notifyListeners();
  }

  Future<void> initialize(String address) async {
    myAddress = address;
    initialized = false;
    log.clear();
    notifyListeners();

    await Future.delayed(const Duration(milliseconds: 30));

    _log('Core init...');
    coreInit();

    for (final name in ['storage_memory', 'crypto_aes', 'transport_ntfy']) {
      _log('Loading $name...');
      try {
        final wasm = await rootBundle.load('assets/plugins/$name.wasm');
        final toml = await rootBundle.loadString('assets/plugins/$name.manifest.toml');
        final manifest = toml.trimLeft()
            .replaceAll('\r\n', '\n')
            .replaceAll('\r', '\n');

        await loadPlugin(
          wasm: wasm.buffer.asUint8List().toList(),
          manifest: manifest,
        );
        _log('$name OK');
      } catch (e) {
        final msg = e.toString();
        if (msg.contains('already loaded')) {
          _log('$name already loaded');
        } else {
          _log('$name FAILED: $msg');
        }
      }
    }

    _log('Configuring transport...');
    try {
      await configureTransport(address: address);
      _log('Transport OK');
    } catch (e) {
      _log('Transport skipped: $e');
    }

    initialized = true;
    _log('Ready!');
    notifyListeners();
  }
}

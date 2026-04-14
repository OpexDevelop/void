import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../state/app_state.dart';
import '../state/chat_state.dart';
import '../state/plugins_state.dart';

class SetupScreen extends StatefulWidget {
  const SetupScreen({super.key});

  @override
  State<SetupScreen> createState() => _SetupScreenState();
}

class _SetupScreenState extends State<SetupScreen> {
  final _addressController = TextEditingController();
  bool _loading = false;
  String? _error;
  final List<String> _log = [];

  void _addLog(String msg) {
    if (mounted) setState(() => _log.add(msg));
  }

  @override
  void dispose() {
    _addressController.dispose();
    super.dispose();
  }

  Future<void> _start() async {
    final address = _addressController.text.trim();
    if (address.isEmpty) {
      setState(() => _error = 'Enter your address');
      return;
    }

    setState(() {
      _loading = true;
      _error = null;
      _log.clear();
    });

    try {
      _addLog('1. coreInit()...');
      final app = context.read<AppState>();
      await app.initializeWithLog(address: address, onLog: _addLog);

      if (!mounted) return;

      if (app.error != null) {
        setState(() {
          _error = app.error;
          _loading = false;
        });
        return;
      }

      _addLog('OK — opening app...');
      await Future.delayed(const Duration(milliseconds: 300));

      if (!mounted) return;
      context.read<ChatState>().init();
      context.read<PluginsState>().refresh();
      Navigator.of(context).pop();
    } catch (e, st) {
      setState(() {
        _error = '$e\n\n$st';
        _loading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Start Messenger'),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            TextField(
              controller: _addressController,
              decoration: const InputDecoration(
                labelText: 'Your address',
                hintText: 'opex777',
                border: OutlineInputBorder(),
              ),
              autocorrect: false,
              enableSuggestions: false,
              onSubmitted: (_) => _loading ? null : _start(),
            ),
            if (_log.isNotEmpty) ...[
              const SizedBox(height: 12),
              Container(
                width: double.maxFinite,
                constraints: const BoxConstraints(maxHeight: 250),
                decoration: BoxDecoration(
                  color: Colors.black87,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: SingleChildScrollView(
                  padding: const EdgeInsets.all(8),
                  child: SelectableText(
                    _log.join('\n'),
                    style: const TextStyle(
                      fontSize: 11,
                      fontFamily: 'monospace',
                      color: Colors.greenAccent,
                    ),
                  ),
                ),
              ),
            ],
            if (_error != null) ...[
              const SizedBox(height: 12),
              Container(
                width: double.maxFinite,
                constraints: const BoxConstraints(maxHeight: 200),
                decoration: BoxDecoration(
                  color: Colors.red.shade900.withOpacity(0.4),
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(color: Colors.red),
                ),
                child: SingleChildScrollView(
                  padding: const EdgeInsets.all(8),
                  child: SelectableText(
                    _error!,
                    style: const TextStyle(
                      fontSize: 11,
                      fontFamily: 'monospace',
                      color: Colors.redAccent,
                    ),
                  ),
                ),
              ),
            ],
          ],
        ),
      ),
      actions: [
        if (_error != null)
          TextButton(
            onPressed: () => setState(() {
              _error = null;
              _log.clear();
              _loading = false;
            }),
            child: const Text('Retry'),
          ),
        FilledButton(
          onPressed: _loading ? null : _start,
          child: _loading
              ? const SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Text('Start'),
        ),
      ],
    );
  }
}

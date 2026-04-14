limport 'package:flutter/material.dart';
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
    });

    final app = context.read<AppState>();
    await app.initialize(address: address);

    if (!mounted) return;

    if (app.error != null) {
      setState(() {
        _error = app.error;
        _loading = false;
      });
      return;
    }

    context.read<ChatState>().init();
    context.read<PluginsState>().refresh();

    Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Start Messenger'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Enter your address.\n'
            'Share it with others to receive messages.',
          ),
          const SizedBox(height: 4),
          const Text(
            'Example: opex777',
            style: TextStyle(color: Colors.grey, fontSize: 12),
          ),
          const SizedBox(height: 16),
          TextField(
            controller: _addressController,
            decoration: InputDecoration(
              labelText: 'Your address',
              hintText: 'opex777',
              border: const OutlineInputBorder(),
              errorText: _error,
            ),
            autocorrect: false,
            enableSuggestions: false,
            onSubmitted: (_) => _loading ? null : _start(),
          ),
          if (_loading) ...[
            const SizedBox(height: 16),
            const Row(
              children: [
                SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                SizedBox(width: 12),
                Text(
                  'Loading plugins...',
                  style: TextStyle(color: Colors.grey),
                ),
              ],
            ),
          ],
        ],
      ),
      actions: [
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

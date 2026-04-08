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
  final _controller = TextEditingController(text: '7777');
  bool _loading = false;
  String? _error;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  Future<void> _start() async {
    final port = int.tryParse(_controller.text.trim());
    if (port == null || port < 1024 || port > 65535) {
      setState(() => _error = 'Enter valid port (1024–65535)');
      return;
    }

    setState(() {
      _loading = true;
      _error = null;
    });

    final app = context.read<AppState>();
    await app.initialize(port);

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
          const Text('Choose your listen port.\nShare IP:port with others to chat.'),
          const SizedBox(height: 16),
          TextField(
            controller: _controller,
            keyboardType: TextInputType.number,
            decoration: InputDecoration(
              labelText: 'Listen port',
              prefixText: '127.0.0.1:',
              border: const OutlineInputBorder(),
              errorText: _error,
            ),
          ),
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

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
  final _ctrl = TextEditingController();
  bool _running = false;

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  Future<void> _start() async {
    final addr = _ctrl.text.trim();
    if (addr.isEmpty) return;

    setState(() => _running = true);

    final app = context.read<AppState>();
    await app.initialize(addr);

    if (!mounted) return;

    context.read<ChatState>().init();
    context.read<PluginsState>().refresh();

    Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    final app = context.watch<AppState>();

    return AlertDialog(
      title: const Text('Start Messenger'),
      content: SizedBox(
        width: double.maxFinite,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: _ctrl,
              enabled: !_running,
              decoration: const InputDecoration(
                labelText: 'Your address',
                hintText: 'opex777',
                border: OutlineInputBorder(),
              ),
              autocorrect: false,
              onSubmitted: (_) => _running ? null : _start(),
            ),
            if (_running) ...[
              const SizedBox(height: 16),
              // Лог — показывает каждый шаг
              Container(
                width: double.maxFinite,
                constraints: const BoxConstraints(maxHeight: 220),
                decoration: BoxDecoration(
                  color: Colors.black,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: ListView.builder(
                  padding: const EdgeInsets.all(8),
                  shrinkWrap: true,
                  itemCount: app.log.length,
                  itemBuilder: (_, i) {
                    final line = app.log[i];
                    final color = line.startsWith('⚠')
                        ? Colors.orange
                        : line.startsWith('Ready')
                            ? Colors.greenAccent
                            : Colors.white70;
                    return Text(
                      line,
                      style: TextStyle(
                        fontSize: 12,
                        fontFamily: 'monospace',
                        color: color,
                      ),
                    );
                  },
                ),
              ),
            ],
          ],
        ),
      ),
      actions: [
        FilledButton(
          onPressed: _running ? null : _start,
          child: _running
              ? const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Text('Start'),
        ),
      ],
    );
  }
}

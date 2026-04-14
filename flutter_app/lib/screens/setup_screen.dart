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
  List<String> _warnings = [];

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
      _warnings = [];
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

    // Есть предупреждения — показываем их но не блокируем
    if (app.warnings.isNotEmpty) {
      setState(() {
        _warnings = app.warnings;
        _loading = false;
      });

      // Показываем предупреждения и продолжаем через 2 сек
      await Future.delayed(const Duration(seconds: 2));
      if (!mounted) return;
    }

    if (!mounted) return;

    context.read<ChatState>().init();
    context.read<PluginsState>().refresh();

    Navigator.of(context).pop();
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
            const Text(
                'Enter your address.\nShare it with others to receive messages.'),
            const SizedBox(height: 4),
            const Text(
              'Example: opex777',
              style: TextStyle(color: Colors.grey, fontSize: 12),
            ),
            const SizedBox(height: 16),
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
                  Expanded(
                    child: Text(
                      'Loading plugins...',
                      style: TextStyle(color: Colors.grey),
                    ),
                  ),
                ],
              ),
            ],
            // Warnings (не блокирующие)
            if (_warnings.isNotEmpty) ...[
              const SizedBox(height: 12),
              Container(
                constraints: const BoxConstraints(maxHeight: 150),
                decoration: BoxDecoration(
                  color: Colors.orange.shade900.withOpacity(0.3),
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(color: Colors.orange.shade700),
                ),
                child: SingleChildScrollView(
                  padding: const EdgeInsets.all(8),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Text(
                        '⚠ Some plugins failed (offline mode)',
                        style: TextStyle(
                          color: Colors.orange,
                          fontWeight: FontWeight.bold,
                          fontSize: 12,
                        ),
                      ),
                      const SizedBox(height: 4),
                      ..._warnings.map(
                        (w) => Text(
                          w,
                          style: const TextStyle(
                            fontSize: 10,
                            fontFamily: 'monospace',
                            color: Colors.orangeAccent,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ],
            // Фатальная ошибка
            if (_error != null) ...[
              const SizedBox(height: 12),
              Container(
                constraints: const BoxConstraints(maxHeight: 200),
                decoration: BoxDecoration(
                  color: Colors.red.shade900.withOpacity(0.3),
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(color: Colors.red.shade700),
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
        if (_warnings.isNotEmpty && !_loading)
          TextButton(
            onPressed: () {
              context.read<ChatState>().init();
              context.read<PluginsState>().refresh();
              Navigator.of(context).pop();
            },
            child: const Text('Continue anyway'),
          ),
        if (_error == null || _error!.isEmpty)
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
        if (_error != null)
          FilledButton(
            onPressed: () => setState(() {
              _error = null;
              _warnings = [];
            }),
            child: const Text('Retry'),
          ),
      ],
    );
  }
}

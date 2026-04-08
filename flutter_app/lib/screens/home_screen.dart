import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../state/app_state.dart';
import 'chat_screen.dart';
import 'plugins_screen.dart';

class HomeScreen extends StatelessWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final state = context.watch<AppState>();
    final portCtrl = TextEditingController(text: '7777');
    final contactCtrl = TextEditingController(text: '127.0.0.1:8888');

    return Scaffold(
      appBar: AppBar(
        title: const Text('Void Messenger'),
        actions: [
          if (state.initialized)
            IconButton(
              icon: const Icon(Icons.extension),
              onPressed: () => Navigator.push(context, MaterialPageRoute(builder: (_) => const PluginsScreen())),
            )
        ],
      ),
      body: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            if (!state.initialized) ...[
              TextField(controller: portCtrl, decoration: const InputDecoration(labelText: 'Listen Port')),
              const SizedBox(height: 16),
              ElevatedButton(
                onPressed: () => state.initialize(int.parse(portCtrl.text)),
                child: const Text('Start Core'),
              ),
              if (state.error != null) Text(state.error!, style: const TextStyle(color: Colors.red)),
            ] else ...[
              Text('Listening on: ${state.myAddress}', style: Theme.of(context).textTheme.titleLarge),
              const SizedBox(height: 32),
              TextField(controller: contactCtrl, decoration: const InputDecoration(labelText: 'Contact (IP:PORT)')),
              const SizedBox(height: 16),
              ElevatedButton(
                onPressed: () => Navigator.push(context, MaterialPageRoute(builder: (_) => ChatScreen(contact: contactCtrl.text))),
                child: const Text('Open Chat'),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

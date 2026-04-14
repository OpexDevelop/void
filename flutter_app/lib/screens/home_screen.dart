import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';
import '../state/app_state.dart';
import 'setup_screen.dart';
import 'contacts_screen.dart';
import 'plugins_screen.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  int _tab = 0;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!context.read<AppState>().initialized) {
        showDialog(
          context: context,
          barrierDismissible: false,
          builder: (_) => const SetupScreen(),
        );
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final app = context.watch<AppState>();

    return Scaffold(
      appBar: AppBar(
        title: const Text('Void Messenger'),
        actions: [
          if (app.initialized)
            GestureDetector(
              onTap: () {
                Clipboard.setData(ClipboardData(text: app.myAddress));
                ScaffoldMessenger.of(context).showSnackBar(
                  const SnackBar(content: Text('Copied!'), duration: Duration(seconds: 1)),
                );
              },
              child: Padding(
                padding: const EdgeInsets.only(right: 16),
                child: Row(
                  children: [
                    Text(
                      app.myAddress,
                      style: const TextStyle(color: Colors.greenAccent, fontSize: 12),
                    ),
                    const SizedBox(width: 4),
                    const Icon(Icons.copy, size: 12, color: Colors.greenAccent),
                  ],
                ),
              ),
            ),
        ],
      ),
      body: IndexedStack(
        index: _tab,
        children: const [ContactsScreen(), PluginsScreen()],
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _tab,
        onDestinationSelected: (i) => setState(() => _tab = i),
        destinations: const [
          NavigationDestination(icon: Icon(Icons.chat_bubble_outline), label: 'Chats'),
          NavigationDestination(icon: Icon(Icons.extension_outlined), label: 'Plugins'),
        ],
      ),
    );
  }
}

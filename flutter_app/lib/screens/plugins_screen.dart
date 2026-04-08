import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../state/plugins_state.dart';

class PluginsScreen extends StatefulWidget {
  const PluginsScreen({super.key});
  @override
  State<PluginsScreen> createState() => _PluginsScreenState();
}

class _PluginsScreenState extends State<PluginsScreen> {
  @override
  void initState() {
    super.initState();
    Future.microtask(() => context.read<PluginsState>().refresh());
  }

  @override
  Widget build(BuildContext context) {
    final state = context.watch<PluginsState>();
    return Scaffold(
      appBar: AppBar(title: const Text('Plugins')),
      body: Column(
        children: [
          if (state.lastError != null) Text(state.lastError!, style: const TextStyle(color: Colors.red)),
          if (state.loading) const LinearProgressIndicator(),
          Expanded(
            child: ListView.builder(
              itemCount: state.plugins.length,
              itemBuilder: (context, index) {
                final p = state.plugins[index];
                return ListTile(
                  title: Text('${p.name} v${p.version}'),
                  trailing: IconButton(icon: const Icon(Icons.delete, color: Colors.red), onPressed: () => state.unload(p.id)),
                );
              },
            ),
          ),
        ],
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: state.loading ? null : () => state.pickAndInstall(),
        child: const Icon(Icons.add),
      ),
    );
  }
}

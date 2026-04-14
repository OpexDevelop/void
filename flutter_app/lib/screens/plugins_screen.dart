import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../state/plugins_state.dart';
import '../state/app_state.dart';
import '../models/plugin_info.dart';
import '../widgets/plugin_card.dart';

class PluginsScreen extends StatelessWidget {
  const PluginsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final state = context.watch<PluginsState>();
    final app = context.watch<AppState>();

    return Scaffold(
      body: Column(
        children: [
          if (state.lastError != null)
            MaterialBanner(
              content: Text(state.lastError!),
              backgroundColor: Colors.red.shade900,
              actions: [
                TextButton(
                  onPressed: () => context.read<PluginsState>().refresh(),
                  child: const Text('Dismiss'),
                ),
              ],
            ),
          Expanded(
            child: !app.initialized
                ? const Center(child: Text('Start the messenger first'))
                : state.plugins.isEmpty
                    ? Center(
                        child: Column(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            const Icon(Icons.extension_off,
                                size: 64, color: Colors.grey),
                            const SizedBox(height: 12),
                            const Text('No plugins loaded'),
                            const SizedBox(height: 8),
                            const Text(
                              'Select .wasm + .toml together to install',
                              textAlign: TextAlign.center,
                              style: TextStyle(color: Colors.grey),
                            ),
                          ],
                        ),
                      )
                    : ListView.separated(
                        padding: const EdgeInsets.all(12),
                        itemCount: state.plugins.length,
                        separatorBuilder: (_, __) => const SizedBox(height: 8),
                        itemBuilder: (_, i) => PluginCard(
                          plugin: state.plugins[i],
                          onUnload: () =>
                              _confirmUnload(context, state.plugins[i]),
                        ),
                      ),
          ),
        ],
      ),
      floatingActionButton: app.initialized
          ? FloatingActionButton.extended(
              onPressed: state.loading
                  ? null
                  : () => _pickPlugin(context),
              icon: state.loading
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(
                          strokeWidth: 2, color: Colors.white),
                    )
                  : const Icon(Icons.add),
              label: const Text('Add Plugin'),
            )
          : null,
    );
  }

  Future<void> _pickPlugin(BuildContext context) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Row(
          children: [
            Icon(Icons.security, color: Colors.amber),
            SizedBox(width: 8),
            Text('Install Plugin'),
          ],
        ),
        content: const Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Select BOTH files at once:',
              style: TextStyle(fontWeight: FontWeight.bold),
            ),
            SizedBox(height: 8),
            Text('• plugin.wasm'),
            Text('• plugin.manifest.toml'),
            SizedBox(height: 12),
            Text(
              'Hold Ctrl/Cmd to select multiple files.',
              style: TextStyle(color: Colors.grey, fontSize: 12),
            ),
            SizedBox(height: 12),
            Text(
              'Review permissions before trusting any plugin.',
              style: TextStyle(color: Colors.amber),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: const Text('Select files'),
          ),
        ],
      ),
    );

    if (confirmed != true) return;
    if (!context.mounted) return;

    await context.read<PluginsState>().pickAndInstall();
  }

  void _confirmUnload(BuildContext context, PluginInfo plugin) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text('Unload ${plugin.name}?'),
        content: const Text('The plugin will be removed from this session.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () {
              context.read<PluginsState>().unload(plugin.id);
              Navigator.pop(ctx);
            },
            child: const Text(
              'Unload',
              style: TextStyle(color: Colors.red),
            ),
          ),
        ],
      ),
    );
  }
}

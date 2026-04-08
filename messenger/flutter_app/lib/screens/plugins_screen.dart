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
                            const Icon(Icons.extension_off, size: 64, color: Colors.grey),
                            const SizedBox(height: 12),
                            const Text('No plugins loaded'),
                            const SizedBox(height: 8),
                            const Text(
                              'Drop a .wasm + manifest.toml pair\nto add functionality',
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
                          onUnload: () => _confirmUnload(context, state.plugins[i]),
                        ),
                      ),
          ),
        ],
      ),
      floatingActionButton: app.initialized
          ? FloatingActionButton.extended(
              onPressed: state.loading ? null : () => _pickPlugin(context),
              icon: state.loading
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white),
                    )
                  : const Icon(Icons.add),
              label: const Text('Add Plugin'),
            )
          : null,
    );
  }

  Future<void> _pickPlugin(BuildContext context) async {
    final state = context.read<PluginsState>();

    final manifest = await _showPermissionsPreviewDialog(context);
    if (manifest == false) return;

    await state.pickAndInstall();
  }

  Future<bool?> _showPermissionsPreviewDialog(BuildContext context) {
    return showDialog<bool>(
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
            Text('You will pick:'),
            SizedBox(height: 8),
            Text('1. A .wasm plugin file'),
            Text('2. Its manifest.toml (or it auto-detects)'),
            SizedBox(height: 12),
            Text(
              'Review the permissions shown on the plugin card before trusting any plugin.',
              style: TextStyle(color: Colors.amber),
            ),
          ],
        ),
        actions: [
          TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('Cancel')),
          FilledButton(onPressed: () => Navigator.pop(ctx, true), child: const Text('Continue')),
        ],
      ),
    );
  }

  void _confirmUnload(BuildContext context, PluginInfo plugin) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text('Unload ${plugin.name}?'),
        content: const Text('The plugin will be removed from this session.'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(ctx), child: const Text('Cancel')),
          TextButton(
            onPressed: () {
              context.read<PluginsState>().unload(plugin.id);
              Navigator.pop(ctx);
            },
            child: const Text('Unload', style: TextStyle(color: Colors.red)),
          ),
        ],
      ),
    );
  }
}

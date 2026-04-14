import 'package:flutter/material.dart';
import '../models/plugin_info.dart';
import 'permission_badge.dart';

class PluginCard extends StatefulWidget {
  final PluginInfo plugin;
  final VoidCallback onUnload;

  const PluginCard({super.key, required this.plugin, required this.onUnload});

  @override
  State<PluginCard> createState() => _PluginCardState();
}

class _PluginCardState extends State<PluginCard> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final p = widget.plugin;

    return Card(
      child: Column(
        children: [
          ListTile(
            leading: CircleAvatar(
              backgroundColor: _categoryColor(p.category).withOpacity(0.2),
              child: Text(p.categoryIcon, style: const TextStyle(fontSize: 18)),
            ),
            title: Row(
              children: [
                Expanded(
                  child: Text(
                    p.name,
                    style: const TextStyle(fontWeight: FontWeight.bold),
                  ),
                ),
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                  decoration: BoxDecoration(
                    color: _categoryColor(p.category).withOpacity(0.2),
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Text(
                    p.category,
                    style: TextStyle(
                      fontSize: 11,
                      color: _categoryColor(p.category),
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                ),
              ],
            ),
            subtitle: Text('v${p.version} · ${p.description}', maxLines: 2),
            trailing: IconButton(
              icon: Icon(_expanded ? Icons.expand_less : Icons.expand_more),
              onPressed: () => setState(() => _expanded = !_expanded),
            ),
          ),
          if (_expanded) ...[
            const Divider(height: 1),
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 4),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Permissions',
                    style: Theme.of(context)
                        .textTheme
                        .labelMedium
                        ?.copyWith(color: Colors.grey),
                  ),
                  const SizedBox(height: 8),
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    children: [
                      PermissionBadge(
                          label: 'Network', granted: p.permissions.network),
                      PermissionBadge(
                          label: 'Filesystem',
                          granted: p.permissions.filesystem),
                      PermissionBadge(
                          label: 'Contacts', granted: p.permissions.contacts),
                      PermissionBadge(
                          label: 'Clipboard',
                          granted: p.permissions.clipboard),
                      PermissionBadge(
                          label: 'Notifications',
                          granted: p.permissions.notifications),
                    ],
                  ),
                  const SizedBox(height: 12),
                  Align(
                    alignment: Alignment.centerRight,
                    child: TextButton.icon(
                      onPressed: widget.onUnload,
                      icon: const Icon(Icons.delete_outline, color: Colors.red),
                      label: const Text(
                        'Unload',
                        style: TextStyle(color: Colors.red),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }

  Color _categoryColor(String category) {
    switch (category) {
      case 'storage':
        return Colors.blue;
      case 'crypto':
        return Colors.green;
      case 'transport':
        return Colors.orange;
      case 'ui':
        return Colors.purple;
      default:
        return Colors.grey;
    }
  }
}

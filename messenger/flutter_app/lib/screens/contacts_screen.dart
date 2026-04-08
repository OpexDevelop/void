import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../state/chat_state.dart';
import '../state/app_state.dart';
import 'chat_screen.dart';

class ContactsScreen extends StatelessWidget {
  const ContactsScreen({super.key});

  void _showAddDialog(BuildContext context) {
    final addrCtrl = TextEditingController();
    final nameCtrl = TextEditingController();

    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Add Contact'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: addrCtrl,
              decoration: const InputDecoration(
                labelText: 'Address',
                hintText: '127.0.0.1:8888',
                border: OutlineInputBorder(),
              ),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: nameCtrl,
              decoration: const InputDecoration(
                labelText: 'Name (optional)',
                border: OutlineInputBorder(),
              ),
            ),
          ],
        ),
        actions: [
          TextButton(onPressed: () => Navigator.pop(ctx), child: const Text('Cancel')),
          FilledButton(
            onPressed: () {
              final addr = addrCtrl.text.trim();
              if (addr.isNotEmpty) {
                final name = nameCtrl.text.trim().isEmpty ? addr : nameCtrl.text.trim();
                context.read<ChatState>().addContact(addr, name);
                Navigator.pop(ctx);
              }
            },
            child: const Text('Add'),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final chat = context.watch<ChatState>();
    final app = context.watch<AppState>();

    if (!app.initialized) {
      return const Center(
        child: Text('Start the messenger to begin chatting'),
      );
    }

    return Scaffold(
      body: chat.contacts.isEmpty
          ? Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  const Icon(Icons.chat_bubble_outline, size: 64, color: Colors.grey),
                  const SizedBox(height: 12),
                  const Text('No contacts yet'),
                  const SizedBox(height: 8),
                  FilledButton.icon(
                    onPressed: () => _showAddDialog(context),
                    icon: const Icon(Icons.add),
                    label: const Text('Add contact'),
                  ),
                ],
              ),
            )
          : ListView.separated(
              itemCount: chat.contacts.length,
              separatorBuilder: (_, __) => const Divider(height: 1),
              itemBuilder: (ctx, i) {
                final contact = chat.contacts[i];
                final msgs = chat.messagesFor(contact.address);
                final last = msgs.isEmpty ? null : msgs.last;
                return ListTile(
                  leading: CircleAvatar(
                    backgroundColor: Theme.of(context).colorScheme.primaryContainer,
                    child: Text(
                      contact.name[0].toUpperCase(),
                      style: const TextStyle(fontWeight: FontWeight.bold),
                    ),
                  ),
                  title: Text(contact.name),
                  subtitle: Text(
                    last?.text ?? contact.address,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  trailing: last != null
                      ? Text(
                          _formatTime(last.timestamp),
                          style: Theme.of(context).textTheme.labelSmall,
                        )
                      : null,
                  onTap: () {
                    chat.openChat(contact.address);
                    Navigator.push(
                      context,
                      MaterialPageRoute(
                        builder: (_) => ChatScreen(contact: contact),
                      ),
                    );
                  },
                  onLongPress: () {
                    showDialog(
                      context: context,
                      builder: (ctx) => AlertDialog(
                        title: Text('Remove ${contact.name}?'),
                        actions: [
                          TextButton(
                            onPressed: () => Navigator.pop(ctx),
                            child: const Text('Cancel'),
                          ),
                          TextButton(
                            onPressed: () {
                              chat.removeContact(contact.address);
                              Navigator.pop(ctx);
                            },
                            child: const Text('Remove', style: TextStyle(color: Colors.red)),
                          ),
                        ],
                      ),
                    );
                  },
                );
              },
            ),
      floatingActionButton: app.initialized
          ? FloatingActionButton(
              onPressed: () => _showAddDialog(context),
              child: const Icon(Icons.add),
            )
          : null,
    );
  }

  String _formatTime(DateTime dt) {
    final now = DateTime.now();
    if (dt.day == now.day) {
      return '${dt.hour.toString().padLeft(2, '0')}:${dt.minute.toString().padLeft(2, '0')}';
    }
    return '${dt.day}.${dt.month}';
  }
}

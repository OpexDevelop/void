import 'dart:async';
import 'package:flutter/foundation.dart';
import '../src/rust/api.dart';
import '../models/message.dart';
import '../models/contact.dart';

class ChatState extends ChangeNotifier {
  final List<Contact> contacts = [];
  final Map<String, List<Message>> _msgs = {};
  bool sending = false;
  Timer? _pollTimer;
  int _lastTs = 0;

  void init() {
    _pollTimer?.cancel();
    _pollTimer = Timer.periodic(const Duration(seconds: 3), (_) => _poll());
  }

  void _poll() {
    final count = pollTransport(sinceTs: _lastTs);
    if (count > 0) {
      final events = pollEvents();
      for (final e in events) {
        if (e.kind == 'message_received') {
          final key = e.from;
          (_msgs[key] ??= []).add(Message(
            from: e.from,
            to: 'me',
            text: e.text,
            timestamp: DateTime.fromMillisecondsSinceEpoch(e.timestamp.toInt() * 1000),
          ));
          if (e.timestamp > _lastTs) _lastTs = e.timestamp.toInt();
        }
      }
      notifyListeners();
    }
  }

  void addContact(String address, String name) {
    if (!contacts.any((c) => c.address == address)) {
      contacts.add(Contact(address: address, name: name));
      notifyListeners();
    }
  }

  void removeContact(String address) {
    contacts.removeWhere((c) => c.address == address);
    notifyListeners();
  }

  void openChat(String address) {
    final msgs = getMessages(contact: address);
    _msgs[address] = msgs.map((m) => Message(
      from: m.from,
      to: m.to,
      text: m.text,
      timestamp: DateTime.fromMillisecondsSinceEpoch(m.timestamp.toInt() * 1000),
    )).toList();
    notifyListeners();
  }

  Future<bool> sendMessage(String to, String text) async {
    sending = true;
    notifyListeners();

    bool ok = false;
    try {
      await sendMessageRust(to: to, text: text);
      (_msgs[to] ??= []).add(Message(
        from: 'me',
        to: to,
        text: text,
        timestamp: DateTime.now(),
      ));
      ok = true;
    } catch (e) {
      debugPrint('[Chat] send failed: $e');
    }

    sending = false;
    notifyListeners();
    return ok;
  }

  List<Message> messagesFor(String addr) =>
      List.unmodifiable(_msgs[addr] ?? []);

  @override
  void dispose() {
    _pollTimer?.cancel();
    super.dispose();
  }
}

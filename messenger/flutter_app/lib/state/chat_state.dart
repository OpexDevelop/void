import 'dart:async';
import 'package:flutter/foundation.dart';
import '../ffi/core_bridge.dart';
import '../models/message.dart';
import '../models/contact.dart';

class ChatState extends ChangeNotifier {
  final List<Contact> contacts = [];
  final Map<String, List<Message>> _messages = {};
  String? _activeContact;
  bool _sending = false;
  StreamSubscription<Message>? _sub;

  String? get activeContact => _activeContact;
  bool get sending => _sending;

  List<Message> messagesFor(String addr) =>
      List.unmodifiable(_messages[addr] ?? []);

  void init() {
    _sub?.cancel();
    _sub = CoreBridge.instance.messageStream.listen((msg) {
      final key = msg.from == 'me' ? msg.to : msg.from;
      _messages.putIfAbsent(key, () => []);
      _messages[key]!.add(msg);
      notifyListeners();
    });
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
    _activeContact = address;
    _loadMessages(address);
    notifyListeners();
  }

  void _loadMessages(String addr) {
    final msgs = CoreBridge.instance.getMessages(addr);
    _messages[addr] = msgs;
  }

  Future<bool> sendMessage(String to, String text) async {
    _sending = true;
    notifyListeners();

    final ok = await CoreBridge.instance.sendMessage(to, text);
    if (ok) {
      _messages.putIfAbsent(to, () => []);
      _messages[to]!.add(Message(
        from: 'me',
        to: to,
        text: text,
        timestamp: DateTime.now(),
      ));
    }

    _sending = false;
    notifyListeners();
    return ok;
  }

  @override
  void dispose() {
    _sub?.cancel();
    super.dispose();
  }
}

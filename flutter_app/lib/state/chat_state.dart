import 'dart:async';
import 'package:flutter/foundation.dart';
import '../ffi/core_bridge.dart';
import '../models/message.dart';
import '../models/contact.dart';

class ChatState extends ChangeNotifier {
  final List<Contact> contacts = [];
  final Map<String, List<Message>> _msgs = {};
  bool sending = false;
  StreamSubscription<Message>? _sub;

  List<Message> messagesFor(String addr) => List.unmodifiable(_msgs[addr] ?? []);

  void init() {
    _sub?.cancel();
    _sub = CoreBridge.instance.messageStream.listen((msg) {
      final key = msg.from == 'me' ? msg.to : msg.from;
      (_msgs[key] ??= []).add(msg);
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
    _msgs[address] = CoreBridge.instance.getMessages(address);
    notifyListeners();
  }

  Future<bool> sendMessage(String to, String text) async {
    sending = true;
    notifyListeners();

    final ok = CoreBridge.instance.sendMessage(to, text);
    if (ok) {
      (_msgs[to] ??= []).add(Message(
        from: 'me',
        to: to,
        text: text,
        timestamp: DateTime.now(),
      ));
    }

    sending = false;
    notifyListeners();
    return ok;
  }

  @override
  void dispose() {
    _sub?.cancel();
    super.dispose();
  }
}

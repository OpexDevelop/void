import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'core_bridge.dart';

class ChatScreen extends StatefulWidget {
  const ChatScreen({super.key});

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  VoidCore? _core;
  Timer? _timer;
  bool _connected = false;

  final _msgCtrl = TextEditingController();
  final _portCtrl = TextEditingController(text: '9001');
  final _peerCtrl = TextEditingController(text: '192.168.1.100:9002');
  final _scrollCtrl = ScrollController();
  final List<Map<String, dynamic>> _msgs = [];

  void _connect() {
    final port = int.tryParse(_portCtrl.text) ?? 9001;
    _core = VoidCore.init(port: port, key: List.filled(32, 0x42));
    _connected = true;

    _timer = Timer.periodic(const Duration(milliseconds: 200), (_) {
      final msg = _core?.poll();
      if (msg != null) {
        setState(() => _msgs.add(msg));
        _scroll();
      }
    });
    setState(() {});
  }

  void _send() {
    final text = _msgCtrl.text.trim();
    if (text.isEmpty || _core == null) return;

    _core!.sendMessage('chat', text, _peerCtrl.text);
    setState(() => _msgs.add({'text': text, 'incoming': false}));
    _msgCtrl.clear();
    _scroll();
  }

  void _scroll() {
    Future.delayed(const Duration(milliseconds: 50), () {
      if (_scrollCtrl.hasClients) {
        _scrollCtrl.animateTo(
          _scrollCtrl.position.maxScrollExtent,
          duration: const Duration(milliseconds: 100),
          curve: Curves.easeOut,
        );
      }
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.black,
      appBar: AppBar(
        title: const Text('Void', style: TextStyle(color: Colors.white)),
        backgroundColor: Colors.grey[900],
      ),
      body: Column(
        children: [
          // --- Подключение ---
          if (!_connected)
            Padding(
              padding: const EdgeInsets.all(16),
              child: Column(children: [
                TextField(
                  controller: _portCtrl,
                  style: const TextStyle(color: Colors.white),
                  decoration: _deco('Мой порт'),
                  keyboardType: TextInputType.number,
                ),
                const SizedBox(height: 8),
                TextField(
                  controller: _peerCtrl,
                  style: const TextStyle(color: Colors.white),
                  decoration: _deco('Адрес пира (ip:port)'),
                ),
                const SizedBox(height: 12),
                SizedBox(
                  width: double.infinity,
                  child: ElevatedButton(
                    onPressed: _connect,
                    style: ElevatedButton.styleFrom(backgroundColor: Colors.deepPurple),
                    child: const Text('Подключиться'),
                  ),
                ),
              ]),
            ),

          // --- Сообщения ---
          Expanded(
            child: ListView.builder(
              controller: _scrollCtrl,
              padding: const EdgeInsets.all(12),
              itemCount: _msgs.length,
              itemBuilder: (_, i) {
                final m = _msgs[i];
                final inc = m['incoming'] == true;
                return Align(
                  alignment: inc ? Alignment.centerLeft : Alignment.centerRight,
                  child: Container(
                    margin: const EdgeInsets.symmetric(vertical: 3),
                    padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 9),
                    decoration: BoxDecoration(
                      color: inc ? Colors.grey[800] : Colors.deepPurple,
                      borderRadius: BorderRadius.circular(16),
                    ),
                    child: Text(
                      m['text'] ?? '',
                      style: const TextStyle(color: Colors.white, fontSize: 15),
                    ),
                  ),
                );
              },
            ),
          ),

          // --- Ввод ---
          if (_connected)
            Container(
              color: Colors.grey[900],
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
              child: Row(children: [
                Expanded(
                  child: TextField(
                    controller: _msgCtrl,
                    style: const TextStyle(color: Colors.white),
                    decoration: _deco('Сообщение...'),
                    onSubmitted: (_) => _send(),
                  ),
                ),
                IconButton(
                  icon: const Icon(Icons.send, color: Colors.deepPurple),
                  onPressed: _send,
                ),
              ]),
            ),
        ],
      ),
    );
  }

  InputDecoration _deco(String hint) => InputDecoration(
    hintText: hint,
    hintStyle: const TextStyle(color: Colors.white30),
    isDense: true,
    contentPadding: const EdgeInsets.symmetric(horizontal: 10, vertical: 10),
    border: OutlineInputBorder(borderRadius: BorderRadius.circular(8)),
    enabledBorder: OutlineInputBorder(
      borderRadius: BorderRadius.circular(8),
      borderSide: BorderSide(color: Colors.grey[700]!),
    ),
  );
}

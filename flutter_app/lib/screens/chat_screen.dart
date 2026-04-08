import 'package:flutter/material.dart';
import '../ffi/core_bridge.dart';
import '../models/message.dart';

class ChatScreen extends StatefulWidget {
  final String contact;
  const ChatScreen({super.key, required this.contact});
  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final _textController = TextEditingController();
  List<Message> _messages = [];

  @override
  void initState() {
    super.initState();
    _loadMessages();
    CoreBridge.instance.messageStream.listen((_) => _loadMessages());
  }

  void _loadMessages() {
    setState(() => _messages = CoreBridge.instance.getMessages(widget.contact));
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(widget.contact)),
      body: Column(
        children: [
          Expanded(
            child: ListView.builder(
              itemCount: _messages.length,
              itemBuilder: (context, index) {
                final msg = _messages[index];
                final isMe = msg.from == 'me';
                return ListTile(
                  title: Align(
                    alignment: isMe ? Alignment.centerRight : Alignment.centerLeft,
                    child: Container(
                      padding: const EdgeInsets.all(8),
                      color: isMe ? Colors.blue.withOpacity(0.2) : Colors.grey.withOpacity(0.2),
                      child: Text(msg.text),
                    ),
                  ),
                );
              },
            ),
          ),
          Padding(
            padding: const EdgeInsets.all(8.0),
            child: Row(
              children: [
                Expanded(child: TextField(controller: _textController)),
                IconButton(
                  icon: const Icon(Icons.send),
                  onPressed: () async {
                    if (_textController.text.isEmpty) return;
                    await CoreBridge.instance.sendMessage(widget.contact, _textController.text);
                    _textController.clear();
                    _loadMessages();
                  },
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

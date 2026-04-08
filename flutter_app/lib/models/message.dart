class Message {
  final String from;
  final String to;
  final String text;
  final DateTime timestamp;

  Message({required this.from, required this.to, required this.text, required this.timestamp});

  factory Message.fromJson(Map<String, dynamic> json) {
    return Message(
      from: json['from'] as String,
      to: json['to'] as String,
      text: json['text'] as String,
      timestamp: DateTime.fromMillisecondsSinceEpoch(((json['timestamp'] as num?) ?? 0).toInt() * 1000),
    );
  }
}

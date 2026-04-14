class Message {
  final String from;
  final String to;
  final String text;
  final DateTime timestamp;

  const Message({
    required this.from,
    required this.to,
    required this.text,
    required this.timestamp,
  });

  factory Message.fromJson(Map<String, dynamic> json) {
    return Message(
      from: json['from'] as String? ?? 'unknown',
      to: json['to'] as String? ?? 'me',
      text: json['text'] as String? ?? '',
      timestamp: DateTime.fromMillisecondsSinceEpoch(
        ((json['timestamp'] as num?) ?? 0).toInt() * 1000,
      ),
    );
  }

  bool isOutgoing(String myAddr) =>
      from == 'me' || from == myAddr || from.isEmpty;
}

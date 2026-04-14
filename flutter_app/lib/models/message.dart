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

  bool get isOutgoing => from == 'me';
}

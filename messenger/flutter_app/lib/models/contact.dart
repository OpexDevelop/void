class Contact {
  final String address;
  final String name;

  const Contact({required this.address, required this.name});

  Map<String, dynamic> toJson() => {'address': address, 'name': name};
}

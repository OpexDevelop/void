class Contact {
  final String address;
  final String name;

  const Contact({required this.address, required this.name});

  Map<String, dynamic> toJson() => {'address': address, 'name': name};

  factory Contact.fromJson(Map<String, dynamic> json) {
    return Contact(
      address: json['address'] as String,
      name: json['name'] as String,
    );
  }
}

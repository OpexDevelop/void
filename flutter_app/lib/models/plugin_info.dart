class PluginInfo {
  final String id;
  final String name;
  final String version;
  final String description;
  final String author;
  final String category;

  PluginInfo({
    required this.id, required this.name, required this.version, 
    required this.description, required this.author, required this.category
  });

  factory PluginInfo.fromJson(Map<String, dynamic> json) {
    return PluginInfo(
      id: json['id'] as String,
      name: json['name'] as String,
      version: json['version'] as String,
      description: json['description'] as String,
      author: json['author'] as String,
      category: json['category'] as String,
    );
  }
}

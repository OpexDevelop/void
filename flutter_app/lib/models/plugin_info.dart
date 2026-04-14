class PluginInfo {
  final String id;
  final String name;
  final String version;
  final String category;
  final String description;
  final bool active;
  final bool network;
  final bool filesystem;

  const PluginInfo({
    required this.id,
    required this.name,
    required this.version,
    required this.category,
    required this.description,
    required this.active,
    required this.network,
    required this.filesystem,
  });

  String get categoryIcon => switch (category) {
    'storage'   => '🗄',
    'crypto'    => '🔐',
    'transport' => '📡',
    'ui'        => '🎨',
    _           => '🔌',
  };
}

class PluginPermissions {
  final bool network;
  final bool filesystem;
  final bool contacts;
  final bool clipboard;
  final bool notifications;

  const PluginPermissions({
    required this.network,
    required this.filesystem,
    required this.contacts,
    required this.clipboard,
    required this.notifications,
  });

  factory PluginPermissions.fromJson(Map<String, dynamic> json) {
    return PluginPermissions(
      network: json['network'] as bool? ?? false,
      filesystem: json['filesystem'] as bool? ?? false,
      contacts: json['contacts'] as bool? ?? false,
      clipboard: json['clipboard'] as bool? ?? false,
      notifications: json['notifications'] as bool? ?? false,
    );
  }
}

class PluginInfo {
  final String id;
  final String name;
  final String version;
  final String category;
  final String description;
  final bool active;
  final PluginPermissions permissions;

  const PluginInfo({
    required this.id,
    required this.name,
    required this.version,
    required this.category,
    required this.description,
    required this.active,
    required this.permissions,
  });

  factory PluginInfo.fromJson(Map<String, dynamic> json) {
    return PluginInfo(
      id: json['id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      version: json['version'] as String? ?? '0.0.0',
      category: json['category'] as String? ?? 'unknown',
      description: json['description'] as String? ?? '',
      active: json['active'] as bool? ?? true,
      permissions: json['permissions'] != null
          ? PluginPermissions.fromJson(
              json['permissions'] as Map<String, dynamic>)
          : const PluginPermissions(
              network: false,
              filesystem: false,
              contacts: false,
              clipboard: false,
              notifications: false,
            ),
    );
  }

  String get categoryIcon {
    switch (category) {
      case 'storage':
        return '🗄';
      case 'crypto':
        return '🔐';
      case 'transport':
        return '📡';
      case 'ui':
        return '🎨';
      default:
        return '🔌';
    }
  }
}

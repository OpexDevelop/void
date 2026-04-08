import 'package:flutter/material.dart';

class PermissionBadge extends StatelessWidget {
  final String label;
  final bool granted;

  const PermissionBadge({
    super.key,
    required this.label,
    required this.granted,
  });

  @override
  Widget build(BuildContext context) {
    final color = granted ? Colors.red : Colors.grey;
    final icon = granted ? Icons.check_circle : Icons.cancel_outlined;

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: color.withOpacity(0.1),
        border: Border.all(color: color.withOpacity(0.4)),
        borderRadius: BorderRadius.circular(20),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 14, color: color),
          const SizedBox(width: 4),
          Text(
            label,
            style: TextStyle(fontSize: 12, color: color),
          ),
        ],
      ),
    );
  }
}

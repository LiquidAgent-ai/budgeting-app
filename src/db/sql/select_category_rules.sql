SELECT id, pattern, match_field, category, tags, is_subscription, priority, enabled, created_at
FROM category_rules
ORDER BY priority ASC, created_at ASC

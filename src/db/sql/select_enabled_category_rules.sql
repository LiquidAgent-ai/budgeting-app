SELECT id, pattern, match_field, category, tags, is_subscription, priority, enabled, created_at
FROM category_rules
WHERE enabled = 1
ORDER BY priority ASC, created_at ASC

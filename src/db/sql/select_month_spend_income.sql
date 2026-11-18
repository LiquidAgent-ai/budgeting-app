SELECT
  COALESCE(SUM(CASE WHEN amount > 0 AND category NOT IN ('Transfer','Income','Investments') THEN amount ELSE 0 END), 0),
  COALESCE(SUM(CASE WHEN amount < 0 AND category = 'Income' THEN ABS(amount) ELSE 0 END), 0)
FROM transactions WHERE date >= ?

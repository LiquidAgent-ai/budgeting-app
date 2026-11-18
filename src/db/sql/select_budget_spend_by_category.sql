SELECT t.category, COALESCE(SUM(t.amount),0)
FROM transactions t
INNER JOIN budgets b ON b.category = t.category
WHERE t.date >= ? AND t.amount > 0
GROUP BY t.category

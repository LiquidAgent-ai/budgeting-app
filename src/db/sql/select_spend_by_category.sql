SELECT category, SUM(amount) as amount
FROM transactions
WHERE date >= ? AND amount > 0 AND category NOT IN ('Transfer','Income','Investments')
GROUP BY category
ORDER BY amount DESC
LIMIT 8

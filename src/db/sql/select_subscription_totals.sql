SELECT COUNT(DISTINCT COALESCE(merchant_name, name)),
       COALESCE(SUM(amount),0)
FROM transactions
WHERE is_subscription = 1 AND amount > 0 AND date >= ?

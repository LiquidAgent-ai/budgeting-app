SELECT t.id, t.account_id, t.plaid_transaction_id, t.amount, t.date, t.name,
       t.merchant_name, t.category, t.subcategory, t.pending, t.is_subscription,
       t.notes, t.created_at, a.name
FROM transactions t
JOIN accounts a ON a.id = t.account_id
WHERE t.is_subscription = 1 AND t.amount > 0 AND t.date >= ?
ORDER BY t.date DESC

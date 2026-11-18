SELECT t.id, t.account_id, t.plaid_transaction_id, t.amount, t.date, t.name,
       t.merchant_name, t.category, t.subcategory, t.pending, t.is_subscription,
       t.notes, t.created_at, a.name
FROM transactions t
JOIN accounts a ON a.id = t.account_id
ORDER BY t.date DESC, t.created_at DESC
LIMIT ?

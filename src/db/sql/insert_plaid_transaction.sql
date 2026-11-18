INSERT INTO transactions (
  id, account_id, plaid_transaction_id, amount, date, name, merchant_name,
  category, pending, is_subscription
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)

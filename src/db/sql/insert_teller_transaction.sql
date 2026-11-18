INSERT INTO transactions (
  id, account_id, teller_transaction_id, amount, date, name, merchant_name,
  category, pending, is_subscription
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)

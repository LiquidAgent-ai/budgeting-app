INSERT INTO transactions (
  id, account_id, amount, date, name, merchant_name, category, is_subscription, pending
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0)

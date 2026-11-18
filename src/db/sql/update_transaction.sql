UPDATE transactions SET
  amount = ?, date = ?, name = ?, merchant_name = ?,
  category = ?, pending = ?, is_subscription = ?
WHERE id = ?

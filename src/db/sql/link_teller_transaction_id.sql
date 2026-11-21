UPDATE transactions
SET teller_transaction_id = ?
WHERE id = ?
  AND teller_transaction_id IS NULL

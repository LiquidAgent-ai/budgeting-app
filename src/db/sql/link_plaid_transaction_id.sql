UPDATE transactions
SET plaid_transaction_id = ?
WHERE id = ?
  AND plaid_transaction_id IS NULL

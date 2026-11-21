SELECT id, name, merchant_name, amount, date,
       plaid_transaction_id, teller_transaction_id
FROM transactions
WHERE account_id = ?
  AND date >= date(?, '-1 day')
  AND date <= date(?, '+1 day')
  AND ABS(amount - ?) < 0.005

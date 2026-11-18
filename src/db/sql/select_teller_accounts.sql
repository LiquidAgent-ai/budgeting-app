SELECT id, teller_account_id, type FROM accounts
WHERE teller_enrollment_id = ? AND teller_account_id IS NOT NULL

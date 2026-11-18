INSERT INTO accounts (
  id, name, official_name, type, subtype, mask, balance_current, balance_available,
  currency, institution_name, teller_account_id, teller_enrollment_id, is_manual, include_in_net_worth
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 1)

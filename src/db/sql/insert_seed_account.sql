INSERT INTO accounts (
  id, name, official_name, type, subtype, mask, balance_current, balance_available,
  currency, institution_name, is_manual, include_in_net_worth
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'USD', ?, 1, 1)

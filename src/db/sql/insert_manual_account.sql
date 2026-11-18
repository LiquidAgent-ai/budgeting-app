INSERT INTO accounts (
  id, name, type, balance_current, balance_available, currency,
  institution_name, is_manual, include_in_net_worth
) VALUES (?, ?, ?, ?, ?, 'USD', ?, 1, 1)

-- The balance below which the member wants a reminder (minor units, account currency). NULL: none.
ALTER TABLE accounts ADD COLUMN low_balance_minor INTEGER;

-- Who is nominated on each bank account and investment (as registered with the bank, fund house
-- or depository), so a family knows what to claim and by whom. Policies have theirs already.
ALTER TABLE accounts ADD COLUMN nominee TEXT;
ALTER TABLE holdings ADD COLUMN nominee TEXT;

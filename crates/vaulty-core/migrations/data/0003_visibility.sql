-- Who can see each transaction (enforced by the household store, see src/household.rs):
--   private: only the owner
--   shared:  the owner plus the specific family members they picked
--   public:  everyone in the family who has turned on Family sharing
ALTER TABLE transactions ADD COLUMN visibility TEXT NOT NULL DEFAULT 'private'
  CHECK (visibility IN ('private', 'shared', 'public'));
CREATE INDEX IF NOT EXISTS idx_transactions_visibility ON transactions(visibility);

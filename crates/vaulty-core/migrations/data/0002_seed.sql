-- Default categories (marked is_system so they cannot be edited/deleted)
INSERT OR IGNORE INTO user_settings (id) VALUES ('default');
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_food', 'Food & Dining', 'expense', '🍔', '#ef4444', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_transport', 'Transportation', 'expense', '🚗', '#f59e0b', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_shopping', 'Shopping', 'expense', '🛒', '#ec4899', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_bills', 'Bills & Utilities', 'expense', '📱', '#8b5cf6', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_entertainment', 'Entertainment', 'expense', '🎬', '#06b6d4', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_health', 'Health & Fitness', 'expense', '💊', '#10b981', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_education', 'Education', 'expense', '📚', '#3b82f6', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_travel', 'Travel', 'expense', '✈️', '#f97316', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_home', 'Home & Garden', 'expense', '🏠', '#84cc16', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_personal', 'Personal Care', 'expense', '💅', '#d946ef', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_salary', 'Salary', 'income', '💰', '#10b981', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_freelance', 'Freelance', 'income', '💻', '#3b82f6', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_investment', 'Investment', 'income', '📈', '#8b5cf6', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_gift', 'Gift', 'income', '🎁', '#ec4899', 1);
INSERT OR IGNORE INTO categories (id, name, type, icon, color, is_system) VALUES ('cat_refund', 'Refund', 'income', '↩️', '#f59e0b', 1);

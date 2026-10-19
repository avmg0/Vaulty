//! Vaulty core: everything the app does with data, independent of Tauri.
//! The desktop shell (src-tauri) only wires these functions to commands.
pub mod accounts;
pub mod backup;
pub mod budgets;
pub mod calendar;
pub mod cards;
pub mod categories;
mod crypto;
pub use crypto::Key;
pub mod db;
pub mod demo;
pub mod error;
pub mod export;
pub mod history;
pub mod hotkeys;
pub mod household;
pub mod institutions;
pub mod ledger;
pub mod limits;
pub mod market;
pub mod money;
pub mod passwords;
pub mod profiles;
pub mod quick;
pub mod recurring;
pub mod rules;
pub mod rows;
pub mod settings;
pub mod snapshots;
pub mod statements;
pub mod tax;
pub mod transactions;

pub use error::{Error, Result};

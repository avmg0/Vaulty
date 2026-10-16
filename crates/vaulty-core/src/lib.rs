//! Vaulty core: everything the app does with data, independent of Tauri.
//! The desktop shell (src-tauri) only wires these functions to commands.
pub mod accounts;
pub mod cards;
pub mod categories;
mod crypto;
pub use crypto::Key;
pub mod db;
pub mod error;
pub mod export;
pub mod history;
pub mod household;
pub mod institutions;
pub mod ledger;
pub mod market;
pub mod money;
pub mod profiles;
pub mod rows;
pub mod settings;
pub mod tax;
pub mod transactions;

pub use error::{Error, Result};

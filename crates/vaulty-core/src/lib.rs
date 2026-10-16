//! Vaulty core: everything the app does with data, independent of Tauri.
//! The desktop shell (src-tauri) only wires these functions to commands.
pub mod accounts;
pub mod cards;
pub mod categories;
mod crypto;
pub mod db;
pub mod error;
pub mod export;
pub mod household;
pub mod institutions;
pub mod ledger;
pub mod money;
pub mod profiles;
pub mod rows;
pub mod settings;
pub mod transactions;

pub use error::{Error, Result};

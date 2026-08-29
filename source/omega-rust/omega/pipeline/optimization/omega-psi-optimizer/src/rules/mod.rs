//! Built-in Psi optimization rules grouped by exact named pass.
//!
//! The ordered catalog remains the only coordination point. Pass modules own
//! candidate production; independent acceptance remains in
//! `omega-optimization-validation`.

mod catalog;
mod passes;

pub use catalog::{ORDERED_PSI_PASSES, built_in_psi_registries, built_in_psi_registry};
pub use passes::*;

#![forbid(unsafe_code)]

//! Optimizer module role: crate map.
//!
//! Target-neutral Psi optimization vocabulary and canonical selections. Psi
//! owns this catalog because every entry executes before Terminal publication;
//! target and physical optimization identities remain Omega-owned.

mod catalog;
mod selection;

pub use catalog::PRETERMINAL_PSI_PASS_CATALOG;
pub use selection::*;

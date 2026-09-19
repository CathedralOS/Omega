//!
//! `evidence.rs` drives the derivation, `children.rs` derives each child,
//! `provider_custody.rs` checks provider custody, `settlement_identity.rs`
//! names settlements and `hashing.rs` encodes every identity.

mod children;
mod evidence;
mod hashing;
mod provider_custody;
mod settlement_identity;
#[cfg(test)]
mod tests;

pub(crate) use evidence::{NativePhysicalEvidenceDerivation, derive_physical_evidence};

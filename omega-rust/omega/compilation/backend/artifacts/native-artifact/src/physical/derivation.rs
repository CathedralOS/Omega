//! Derive physical evidence by following its source of execution authority.
//!
//! `evidence.rs` drives the artifact-wide derivation. `hosted_builtins.rs` owns
//! the closed builtin catalog, `provider_settlements.rs` owns admitted in-place
//! settlements, `internal_calls.rs` owns selected boundary internal calls, and
//! `foreign_calls.rs` owns normalized foreign-call evidence
//! with fragment custody replay beneath it. `provider_custody.rs` checks shared
//! provider custody, `settlement_identity.rs` names settlements, and `hashing.rs`
//! encodes every identity.

pub(crate) mod evidence;
mod foreign_calls;
pub(crate) mod hashing;
mod hosted_builtins;
mod internal_calls;
mod provider_custody;
mod provider_settlements;
mod settlement_identity;
#[cfg(test)]
mod tests;

pub(crate) use evidence::{NativePhysicalEvidenceDerivation, derive_physical_evidence};

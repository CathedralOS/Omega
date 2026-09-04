#![forbid(unsafe_code)]

//! Self-contained, target-neutral terminal-Psi semantics.
//!
//! Omega and Psi are pre-release, so this crate exposes one current vocabulary
//! rather than a compatibility ladder. It contains explicit values and control,
//! bodyful contracts, structural content facts, crash containment, Boolean and
//! fixed-integer operations, proof-gated exact arithmetic, and guarded runtime
//! arithmetic reconstruction. Extending the vocabulary updates its execution,
//! proof, serialization, fuel, and lowering contracts together; stale artifacts
//! are rejected instead of migrated or assigned enduring semantic versions.

mod dynamic_dispatch;
mod identity;
mod module;
mod observation_profile;
mod proof_values;
mod quotient_correspondence;

pub use psi_language_core::BindingRelevance;

pub use dynamic_dispatch::*;
pub use identity::*;
pub use module::*;
pub use observation_profile::*;
pub use proof_values::*;
pub use quotient_correspondence::*;

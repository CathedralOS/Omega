//! Typed recovery of normalized policy, not recovery of compiler authority.
//!
//! `external` owns the component envelope; `reader` accounts hostile framing
//! and allocations. The other leaves mirror stable signature, contract,
//! expression, identity, and behavior vocabulary without inspecting compiler
//! representations or reconstructing proofs.

mod behavior;
mod conformance;
mod contracts;
mod expressions;
mod external;
mod identity;
mod model;
mod physical_calling_policy;
mod reader;
mod signatures;

use PackagePolicyRecoveryError as Error;
pub use model::{PackagePolicyRecoveryError, PackagePolicyRecoveryLimits};

//! Typed recovery of normalized policy, not recovery of compiler authority.
//!
//! `external` owns the component envelope; `reader` accounts hostile framing
//! and allocations. The other leaves mirror stable signature, contract,
//! expression, identity, and behavior vocabulary without inspecting compiler
//! representations or reconstructing proofs.

mod behavior;
mod callable_policy;
mod calling_application;
mod conformance;
mod contracts;
mod expressions;
mod external;
mod identity;
mod intrinsic;
mod model;
mod physical_calling_policy;
mod reader;
mod representation;
mod selected_providers;
mod signatures;
mod structural_expressions;
mod terminal_permissions;

use PackagePolicyRecoveryError as Error;
pub use model::{PackagePolicyRecoveryError, PackagePolicyRecoveryLimits};

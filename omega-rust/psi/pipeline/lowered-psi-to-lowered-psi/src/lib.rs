#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance.
//!
//! Target-neutral optimization between checked lowering and Terminal
//! publication. Start at `psi_optimization.rs`: the driver consumes the
//! complete unsealed Psi product, runs the selected passes in canonical
//! order, and returns the only carrier accepted by canonical Terminal
//! publication. Each pass folder owns one selectable optimization; the
//! shared `retained_identities` owner records proof and ranking identities
//! that rewrites must preserve. The coordinator alone constructs validated results.

mod control_flow_cleanup;
mod copy_propagation;
mod dead_scalar_elimination;
mod global_value_numbering;
mod optimization_error;
mod proof_check_elision;
mod psi_optimization;
mod retained_identities {
    pub(crate) mod proof_values;
    pub(crate) mod ranked_coverage;
    #[cfg(test)]
    mod tests;
}
mod sparse_conditional_constant_propagation;

pub use optimization_error::PsiOptimizationStageError;
pub use psi_optimization::{PsiOptimizationStageResult, run_psi_optimization};
pub use terminal_codec::{PsiOptimizationExecutionIdentity, PsiOptimizationExecutionRecord};

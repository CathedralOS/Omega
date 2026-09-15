#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance.
//!
//! Target-neutral optimization between checked lowering and Terminal
//! publication. Start at `psi_optimization.rs`: the driver consumes the
//! complete unsealed Psi product, runs the selected passes in canonical
//! order, and returns the only carrier accepted by canonical Terminal
//! publication. Each pass folder owns one selectable optimization; the
//! `psi_optimization` folder owns the stage carriers and the ranked and
//! retained coverage every pass respects.

mod control_flow_cleanup;
mod copy_propagation;
mod dead_scalar_elimination;
mod global_value_numbering;
mod proof_check_elision;
mod psi_optimization;
mod sparse_conditional_constant_propagation;

pub use psi_optimization::{
    PsiOptimizationStageError, PsiOptimizationStageResult, run_psi_optimization,
};
pub use terminal_codec::{PsiOptimizationExecutionIdentity, PsiOptimizationExecutionRecord};

#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance.
//!
//! Target-neutral optimization between checked lowering and Terminal
//! publication.
//!
//! One entrance: [`run_psi_optimization`], in `psi_optimization.rs`. It takes
//! the complete unsealed Psi product, validates the carrier, runs the selected
//! passes, validates the carrier again, and returns the
//! [`PsiOptimizationStageResult`] that canonical Terminal publication accepts.
//! The coordinator alone constructs that validated result; no pass can mint
//! one. The empty selection is not a no-op — it validates both sides of the
//! identity transformation.
//!
//! The pass modules below are in the canonical order of
//! `PRETERMINAL_PSI_PASS_CATALOG`, which is the order the entrance's dispatch
//! states them in and the order a selection executes them in. Each folder owns
//! one selectable optimization and nothing else:
//!
//! 1. `control_flow_cleanup`
//! 2. `sparse_conditional_constant_propagation`
//! 3. `copy_propagation`
//! 4. `global_value_numbering`
//! 5. `dead_scalar_elimination`
//! 6. `proof_check_elision`
//!
//! The catalog's remaining two members, `StateSpecialization` and
//! `RepresentationSpecialization`, have no folder here: they rewrite the
//! abstract-operations unit produced only after this stage, so they execute
//! post-Terminal and are filtered out of this stage's execution record.
//!
//! Beneath the passes, `retained_identities` records the proof and ranking
//! identities a rewrite must preserve — `copy_propagation`,
//! `global_value_numbering` and `dead_scalar_elimination` all consult it —
//! and `optimization_error` carries the refusals the entrance and every pass
//! return.

// The entrance.
mod psi_optimization;

// The selectable passes, in canonical catalog order.
mod control_flow_cleanup;
mod copy_propagation;
mod dead_scalar_elimination;
mod global_value_numbering;
mod proof_check_elision;
mod sparse_conditional_constant_propagation;

// Beneath the passes: what a rewrite must preserve, and how it refuses.
mod optimization_error;
mod retained_identities;

// The entrance and its validated carrier.
pub use psi_optimization::{PsiOptimizationStageResult, run_psi_optimization};

// The refusal a caller of that entrance matches on.
pub use optimization_error::PsiOptimizationStageError;

// The execution record this stage fills: owned by `terminal_codec`, which
// encodes it, and re-exported here beside the entrance that produces it.
pub use terminal_codec::{PsiOptimizationExecutionIdentity, PsiOptimizationExecutionRecord};

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
//! The five implemented passes below execute in the canonical order a
//! selection records them in, which is the order the entrance's dispatch
//! states them in. Each folder owns one selectable optimization and nothing
//! else:
//!
//! 1. `control_flow_cleanup`
//! 2. `sparse_conditional_constant_propagation`
//! 3. `copy_propagation`
//! 4. `global_value_numbering`
//! 5. `dead_scalar_elimination`
//!
//! Four vocabulary members have no folder here: `ProofCheckElision`,
//! `StateSpecialization`, `RepresentationSpecialization` and
//! `LoopInvariantScalarMotion` rewrite the abstract-operations unit produced
//! only after this stage and execute in the post-Terminal stage. The
//! entrance's exhaustive dispatch still names each of them, and a selection
//! containing one executes no rule here.
//!
//! Beneath the passes, `retained_identities` records the proof and ranking
//! identities a rewrite must preserve — `copy_propagation`,
//! `global_value_numbering` and `dead_scalar_elimination` all consult it —
//! and `optimization_error` carries the refusals the entrance and every pass
//! return. `rewrite_refusal` records the deliberate, non-error refusals: a
//! proof-bearing module whose reconstructed proof question a proposed
//! rewrite would change keeps the module as authored, and the stage result
//! lists which pass withheld its rewrite and why.

// The entrance.
mod psi_optimization;

// The five implemented passes; the numbered list above is their canonical
// execution order.
mod control_flow_cleanup;
mod copy_propagation;
mod dead_scalar_elimination;
mod global_value_numbering;
mod sparse_conditional_constant_propagation;

// Beneath the passes: edge arguments, retained identities, and refusals.
mod edge_arguments;
mod optimization_error;
mod retained_identities;

// The entrance and its validated carrier.
pub use psi_optimization::{PsiOptimizationStageResult, run_psi_optimization};

// The refusal a caller of that entrance matches on.
pub use optimization_error::PsiOptimizationStageError;

// The deliberate rewrite withholdings the validated result lists.

// The execution record this stage fills: owned by `terminal_codec`, which
// encodes it, and re-exported here beside the entrance that produces it.
pub use terminal_codec::{PsiOptimizationExecutionIdentity, PsiOptimizationExecutionRecord};

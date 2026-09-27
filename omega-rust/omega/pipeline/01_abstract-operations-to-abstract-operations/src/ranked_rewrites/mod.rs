//! Optimizer module role: stage group. Rewrite boundaries that require authenticated ranked-cycle custody.

#[cfg(any(test, feature = "test-support"))]
mod countdown_invariant_constant_relocation;
pub(crate) mod loop_invariant_scalar_motion;

#[cfg(any(test, feature = "test-support"))]
pub use countdown_invariant_constant_relocation::{
    CountdownInvariantConstantRelocationError, apply_countdown_invariant_constant_relocation,
    propose_countdown_invariant_constant_relocations,
    validate_countdown_invariant_constant_relocation,
};
pub use loop_invariant_scalar_motion::LoopInvariantScalarMotionError;
#[cfg(test)]
pub(crate) use loop_invariant_scalar_motion::{
    LoopInvariantNodeResult, LoopInvariantScalarNode, LoopInvariantScalarRelocation,
};
#[cfg(any(test, feature = "test-support"))]
pub use loop_invariant_scalar_motion::{
    apply_loop_invariant_scalar_motion, propose_loop_invariant_scalar_motion,
    validate_loop_invariant_scalar_motion,
};

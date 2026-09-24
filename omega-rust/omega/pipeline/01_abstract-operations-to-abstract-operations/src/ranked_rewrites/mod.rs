//! Optimizer module role: stage group. Rewrite boundaries that require authenticated ranked-cycle custody.

mod countdown_invariant_constant_relocation;
mod loop_invariant_scalar_motion;

pub use countdown_invariant_constant_relocation::{
    CountdownInvariantConstantRelocationError, apply_countdown_invariant_constant_relocation,
    propose_countdown_invariant_constant_relocations,
    validate_countdown_invariant_constant_relocation,
};
#[cfg(test)]
pub(crate) use loop_invariant_scalar_motion::{
    LoopInvariantNodeResult, LoopInvariantScalarNode, LoopInvariantScalarRelocation,
};
pub use loop_invariant_scalar_motion::{
    LoopInvariantScalarMotionError, apply_loop_invariant_scalar_motion,
    propose_loop_invariant_scalar_motion, validate_loop_invariant_scalar_motion,
};

//! Optimizer module role: stage group. Rewrite boundaries that require authenticated ranked-cycle custody.

mod countdown_invariant_constant_relocation;
mod loop_invariant_scalar_motion;

pub use countdown_invariant_constant_relocation::{
    AppliedCountdownInvariantConstantRelocation, CountdownInvariantConstantRelocation,
    CountdownInvariantConstantRelocationCandidate, CountdownInvariantConstantRelocationError,
    ValidatedCountdownInvariantConstantRelocation, apply_countdown_invariant_constant_relocation,
    propose_countdown_invariant_constant_relocations,
    validate_countdown_invariant_constant_relocation,
};
pub use loop_invariant_scalar_motion::{
    AppliedLoopInvariantScalarMotion, LoopInvariantScalarMotionCandidate,
    LoopInvariantScalarMotionError, LoopInvariantScalarNode, LoopInvariantScalarRelocation,
    ValidatedLoopInvariantScalarMotion, apply_loop_invariant_scalar_motion,
    propose_loop_invariant_scalar_motion, validate_loop_invariant_scalar_motion,
};

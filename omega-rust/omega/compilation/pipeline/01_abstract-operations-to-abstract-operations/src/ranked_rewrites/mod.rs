//! Optimizer module role: stage group. Rewrite boundaries that require authenticated ranked-cycle custody.

pub(crate) mod loop_invariant_scalar_motion;

pub(crate) use loop_invariant_scalar_motion::LoopInvariantScalarMotionError;

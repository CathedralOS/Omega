//! Optimizer module role: stage group. Rewrite boundaries that require authenticated ranked-cycle custody.

mod countdown_invariant_constant_relocation;

pub use countdown_invariant_constant_relocation::{
    apply_countdown_invariant_constant_relocation,
    propose_countdown_invariant_constant_relocations,
    validate_countdown_invariant_constant_relocation, AppliedCountdownInvariantConstantRelocation,
    CountdownInvariantConstantRelocation, CountdownInvariantConstantRelocationCandidate,
    CountdownInvariantConstantRelocationError, ValidatedCountdownInvariantConstantRelocation,
};

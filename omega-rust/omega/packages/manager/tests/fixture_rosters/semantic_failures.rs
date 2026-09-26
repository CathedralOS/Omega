//! Corpus sources checked in root and transitive package review.

pub(crate) const FILE_EXPECTATION_FAIL_CANARIES: &[&str] = &[
    "domains/exit_ensures_unproven",
    "capabilities/effect_ceiling_exceeded",
];

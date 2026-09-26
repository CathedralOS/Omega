//! Shared one-field substitution machinery for custody-mutation matrices.
//!
//! The machinery itself now lives in the shared `mutation-matrix` foundation
//! crate so Psi-side custody families can consume it without crossing the
//! Omega boundary; this module keeps every name at its published
//! `optimization_core` path.

pub use ::mutation_matrix::{
    MutationOutcome, OneFieldSubstitutionMatrix, custody_field_inventory,
    run_one_field_substitution_matrix,
};

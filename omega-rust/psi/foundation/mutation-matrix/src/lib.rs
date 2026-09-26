//! Shared one-field substitution machinery for custody-mutation matrices.
//!
//! A record family declares its canonical field inventory once through
//! `custody_field_inventory!`; the declaration emits the `*FieldForTest`
//! vocabulary and its `INVENTORY`, so the covered field set derives from the
//! recorded receipt instead of a parallel handwritten list. The family also
//! provides an honest-recomputation hook that mutates exactly one field and
//! recomputes any containing identity, and a named independent checker. The
//! test then hands those pieces to `run_one_field_substitution_matrix` — a new
//! family adds a declaration, not another several-hundred-line matrix.

mod mutation_matrix;

pub use mutation_matrix::{
    MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
};

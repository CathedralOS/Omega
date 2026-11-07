// Re-exports of the shared `mutation-matrix` crate under `optimization_core`
// for test-support consumers.

pub use ::mutation_matrix::{
    MutationOutcome, OneFieldSubstitutionMatrix, custody_field_inventory,
    run_one_field_substitution_matrix,
};

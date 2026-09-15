//! Independently checked normalization for ordered same-carrier endpoint maps.
//!
//! This is a certificate prerequisite, not an arithmetic proof rule. It binds
//! a producer's normalized affine or landed-count exact-shift claim to exact,
//! prior semantic-axiom rows so later proof rules do not need to trust an
//! analyzer's coefficients or endpoint arithmetic.
//!
//! `witness_checking.rs` checks the witness, `bound_mapping.rs` maps bounds
//! through it and `truth_bounds.rs` computes what the form admits.

mod bound_mapping;
#[cfg(test)]
mod tests;
mod truth_bounds;
mod witness_checking;

pub use bound_mapping::{
    IntegerAffineBoundConversionError, check_integer_affine_bound_conversion,
    integer_affine_wrapping_evidence, map_integer_affine_bound,
};
pub use terminal_psi::IntegerAffineWitness;
pub use truth_bounds::integer_affine_truth_bounds;
pub use witness_checking::{
    CheckedIntegerAffineForm, IntegerAffineWitnessError, check_integer_affine_witness,
};

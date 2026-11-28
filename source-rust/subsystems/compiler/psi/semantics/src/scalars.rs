/*
Semantic judgments for scalar leaf evaluation, arithmetic and comparison contracts.
*/

pub(crate) mod atomic_update;
pub(crate) mod proof_bearing_scalar;
pub(crate) mod scalar_array;

pub(crate) mod scalar_field_requirements;
pub(crate) mod scalar_leaf_schema;
pub(crate) mod scalar_leaf_semantics;
pub(crate) mod scalar_observations;
pub mod scalar_ordering;
pub(crate) mod unchanged_scalar;

#[cfg(test)]
mod symbolic_integer_widths;

#[cfg(test)]
mod trapping_integer_tests;

//! Typed value expressions, access plans and constant encodings.

pub mod references;
pub mod values;
pub(crate) mod access_views_and_commitments;
pub(crate) mod canonical_values;
pub(crate) mod constant_values;
#[cfg(test)]
mod canonical_const_carrier_tests;

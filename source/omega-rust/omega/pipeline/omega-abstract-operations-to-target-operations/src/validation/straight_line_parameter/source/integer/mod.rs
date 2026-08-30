//! Typed integer-expression source grammar entrance.
//!
//! Exact unary, bitwise, and comparison grammars own their result-envelope
//! choice. This rung owns only their common typed-parameter lookup.

pub(in crate::validation::straight_line_parameter) mod bitwise;
pub(in crate::validation::straight_line_parameter) mod comparison;
pub(in crate::validation::straight_line_parameter) mod unary;

use psi_core::{IntegerType, ScalarType, ValueId};

use super::super::model::ReconstructedEnvelope;

pub(in crate::validation::straight_line_parameter::source) fn parameter(
    envelope: &ReconstructedEnvelope<'_>,
    value: ValueId,
) -> Option<(usize, IntegerType)> {
    envelope
        .parameters
        .iter()
        .enumerate()
        .find_map(|(index, candidate)| match candidate.scalar_type {
            ScalarType::Integer(integer_type) if candidate.value == value => {
                Some((index, integer_type))
            }
            ScalarType::Integer(_) | ScalarType::Boolean => None,
        })
}

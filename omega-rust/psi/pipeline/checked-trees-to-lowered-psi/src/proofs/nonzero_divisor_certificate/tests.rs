//! Fixtures shared by the nonzero divisor certificate tests.

mod byte_subslice;
mod case_analysis;
mod conjunction_endpoints;
mod derived_endpoint_bounds;
mod division_affine_transport;
mod division_citation_composition;
mod live_field_aliases;
mod signed_and_division_goals;
mod subtract_and_multiply_goals;

use crate::proofs::nonzero_divisor_certificate::PropositionContext;
use semantic_vocabulary::{IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId};

fn value(id: u64, integer_type: IntegerType) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(id).expect("value id"),
        ScalarType::Integer(integer_type),
    )
}

fn integer(integer_type: IntegerType, value: i128) -> ScalarTerm {
    ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("integer")
}

fn two_value_context(integer_type: IntegerType) -> PropositionContext {
    PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .unwrap()
}

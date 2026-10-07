//! Fixtures shared by the integer affine tests: scalar values and literals.

mod bound_mapping;
mod witness_checking;
mod wrapping_bounds;
mod wrapping_subtraction;
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::ScalarTerm;

pub(crate) use super::super::value;

fn literal(integer_type: IntegerType, value: i128) -> ScalarTerm {
    ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("literal")
}

fn unsigned_literal(integer_type: IntegerType, value: u128) -> ScalarTerm {
    ScalarTerm::integer(integer_type, IntegerValue::Unsigned(value)).expect("unsigned literal")
}

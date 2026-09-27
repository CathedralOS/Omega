//! Fixtures shared by the integer affine tests: scalar values and literals.

mod bound_mapping;
mod witness_checking;
mod wrapping_bounds;
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::ScalarTerm;
use semantic_vocabulary::ScalarType;
use semantic_vocabulary::ValueId;

fn value(id: u64, integer_type: IntegerType) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(id).expect("value"),
        ScalarType::Integer(integer_type),
    )
}

fn literal(integer_type: IntegerType, value: i128) -> ScalarTerm {
    ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("literal")
}

fn unsigned_literal(integer_type: IntegerType, value: u128) -> ScalarTerm {
    ScalarTerm::integer(integer_type, IntegerValue::Unsigned(value)).expect("unsigned literal")
}

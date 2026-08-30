use super::common::scalar_terminal_artifact;
use super::*;

pub(crate) fn integer_literal_return_artifact(
    integer_type: IntegerType,
    value: IntegerValue,
) -> (Vec<u8>, Vec<u8>) {
    scalar_terminal_artifact(
        ScalarType::Integer(integer_type),
        Vec::new(),
        Some(OperationKind::IntegerConstant { value }),
        None,
        None,
    )
}

pub(crate) fn boolean_literal_return_artifact(value: bool) -> (Vec<u8>, Vec<u8>) {
    scalar_terminal_artifact(
        ScalarType::Boolean,
        Vec::new(),
        Some(OperationKind::BooleanConstant { value }),
        None,
        None,
    )
}

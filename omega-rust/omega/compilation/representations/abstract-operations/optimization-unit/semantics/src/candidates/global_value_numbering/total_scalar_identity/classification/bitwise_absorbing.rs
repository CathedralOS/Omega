//! Independent exact-width bitwise absorbing-literal classification.

use abstract_operations::AbstractOperation as O;
use optimization_unit::TotalScalarIdentityKind;

use super::{
    BitwiseFamily, IndependentTotalScalarIdentity, all_ones, bitwise_operands, row, typed_integer,
};

pub(super) fn classify(
    operation: &O,
    identity: TotalScalarIdentityKind,
) -> Option<IndependentTotalScalarIdentity> {
    use TotalScalarIdentityKind as Kind;
    let operands = bitwise_operands(operation)?;
    let zero = typed_integer(operands.scalar_type, 0);
    let ones = all_ones(operands.scalar_type);
    let (operand, law_constant) = match (operands.family, identity) {
        (BitwiseFamily::And, Kind::IntegerBitwiseAndZeroLeft) => (operands.left, zero),
        (BitwiseFamily::And, Kind::IntegerBitwiseAndZeroRight) => (operands.right, zero),
        (BitwiseFamily::Or, Kind::IntegerBitwiseOrAllOnesLeft) => (operands.left, ones),
        (BitwiseFamily::Or, Kind::IntegerBitwiseOrAllOnesRight) => (operands.right, ones),
        _ => return None,
    };
    Some(row(
        operands.source_operation,
        operands.result,
        operand,
        operand,
        operands.scalar_type,
        operands.scalar_type,
        law_constant,
    ))
}

//! Independent exact-width bitwise neutral-literal classification.

use abstract_operations::AbstractOperation as O;
use optimization_unit::TotalScalarIdentityKind;

use super::{
    BitwiseFamily, BitwiseSide, IndependentTotalScalarIdentity, all_ones, bitwise_law_row,
    bitwise_operands, typed_integer,
};

pub(super) fn classify(
    operation: &O,
    identity: TotalScalarIdentityKind,
) -> Option<IndependentTotalScalarIdentity> {
    use TotalScalarIdentityKind as Kind;
    let operands = bitwise_operands(operation)?;
    let zero = typed_integer(operands.scalar_type, 0);
    let ones = all_ones(operands.scalar_type);
    let (law_side, law_constant) = match (operands.family, identity) {
        (BitwiseFamily::And, Kind::IntegerBitwiseAndAllOnesLeft) => (BitwiseSide::Left, ones),
        (BitwiseFamily::And, Kind::IntegerBitwiseAndAllOnesRight) => (BitwiseSide::Right, ones),
        (BitwiseFamily::Or, Kind::IntegerBitwiseOrZeroLeft) => (BitwiseSide::Left, zero),
        (BitwiseFamily::Or, Kind::IntegerBitwiseOrZeroRight) => (BitwiseSide::Right, zero),
        (BitwiseFamily::Xor, Kind::IntegerBitwiseXorZeroLeft) => (BitwiseSide::Left, zero),
        (BitwiseFamily::Xor, Kind::IntegerBitwiseXorZeroRight) => (BitwiseSide::Right, zero),
        _ => return None,
    };
    Some(bitwise_law_row(operands, law_side, law_constant))
}

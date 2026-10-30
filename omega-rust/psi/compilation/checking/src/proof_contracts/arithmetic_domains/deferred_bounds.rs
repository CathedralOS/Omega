//! Closed comparison endpoints become conditional interval recipes once.
//!
//! Call guarantees and checked conditions select the comparison's exact builtin
//! meaning and subject before entering here. This owner preserves the complete
//! fixed-width carrier, direction and strictness; selected endpoint formation and
//! evaluation remain debts of the ordinary target closure.

use super::{BinaryOperator, ExpressionHandle, PrimitiveType};
use checked_trees::IntegerRangeRecipe;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;

pub(super) fn comparison_recipe(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    primitive: PrimitiveType,
    bound: ExpressionHandle,
    operator: BinaryOperator,
) -> Option<IntegerRangeRecipe> {
    if !program
        .constant_recipe_for_contract_expression(bound)
        .is_valid()
    {
        return None;
    }
    let carrier = super::integer_ranges::primitive_range(primitive)?;
    let low = BigInt::from_i64(carrier.low?);
    let high = carrier
        .high
        .map(BigInt::from_i64)
        .or_else(|| (primitive == PrimitiveType::U64).then(|| BigInt::from_u64(u64::MAX)))?;
    let observation = IntegerRangeRecipe::observation(bound, owner);
    let lower = IntegerRangeRecipe::bounds(low.clone(), low.clone());
    let upper = IntegerRangeRecipe::bounds(high.clone(), high.clone());
    let one = IntegerRangeRecipe::bounds(BigInt::from_i64(1), BigInt::from_i64(1));
    let selected = match operator {
        BinaryOperator::LessOrEqual => IntegerRangeRecipe::interval(&lower, &observation),
        BinaryOperator::Less => IntegerRangeRecipe::interval(
            &lower,
            &observation.binary(BinaryOperator::Subtract, &one)?,
        ),
        BinaryOperator::GreaterOrEqual => IntegerRangeRecipe::interval(&observation, &upper),
        BinaryOperator::Greater => {
            IntegerRangeRecipe::interval(&observation.binary(BinaryOperator::Add, &one)?, &upper)
        }
        BinaryOperator::Equal => observation,
        _ => return None,
    };
    Some(selected.intersect(&IntegerRangeRecipe::bounds(low, high)))
}

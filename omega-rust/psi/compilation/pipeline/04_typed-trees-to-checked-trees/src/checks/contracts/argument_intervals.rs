//! Call requirements proved from the arithmetic walk's argument intervals.
//!
//! Validation's arithmetic walk checks every call argument in the branch-local
//! environment it is evaluated in, where an `&&` or `||` operand and refuted
//! earlier transition arms already narrow the values. A requirement that
//! compares one integer parameter with a literal (`requires index <= 4`) holds
//! at a call when every value the walk allows for that argument satisfies it:
//! `sum(values, index + 1, ..)` after a refuted `index >= 4 ->` arm passes
//! `index + 1` in `[1, 4]`. Conjunctions of such comparisons hold when each
//! does. Every other requirement keeps its existing provers; this is a client
//! of the one interval engine rather than another private one. The call capture
//! owner supplies the exact completed actual for each demanded formal; this
//! consumer never reconstructs source arguments or evaluates them again.
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

pub(super) fn proves(
    program: &dyn ::validation::CheckingInput,
    requirement: ExpressionHandle,
    intervals: &[validation::ExpressionInterval],
    argument_for: &impl Fn(ExpressionHandle) -> Option<ExpressionHandle>,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(requirement) else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        return proves(program, binary.left, intervals, argument_for)
            && proves(program, binary.right, intervals, argument_for);
    }
    let literal = |side: ExpressionHandle| match program.expression_table.expression(side) {
        ExpressionNode::Integer(value) => value.value_i64(),
        _ => None,
    };
    // `parameter OP literal`, or the mirrored `literal OP parameter`.
    let (parameter, operator, bound) = if let Some(bound) = literal(binary.right) {
        (binary.left, binary.operator, bound)
    } else if let Some(bound) = literal(binary.left) {
        let mirrored = binary.operator.mirrored();
        (binary.right, mirrored, bound)
    } else {
        return false;
    };
    let Some(argument) = argument_for(parameter) else {
        return false;
    };
    let Some(interval) = validation::expression_interval(intervals, argument) else {
        return false;
    };
    match operator {
        BinaryOperator::Less => interval.high.is_some_and(|high| high < bound),
        BinaryOperator::LessOrEqual => interval.high.is_some_and(|high| high <= bound),
        BinaryOperator::Greater => interval.low.is_some_and(|low| low > bound),
        BinaryOperator::GreaterOrEqual => interval.low.is_some_and(|low| low >= bound),
        BinaryOperator::Equal => interval.low == Some(bound) && interval.high == Some(bound),
        BinaryOperator::NotEqual => {
            interval.high.is_some_and(|high| high < bound)
                || interval.low.is_some_and(|low| low > bound)
        }
        _ => false,
    }
}

/// A captured integer actual can constrain an exact deferred contract recipe.
/// Retention is conditional evidence: every recorded relation must discharge
/// under the selected target before Terminal production. A partial conjunction
/// publishes nothing, and reference contents never use an argument-time hull.
pub(super) fn recipe_requirements(
    program: &dyn ::validation::CheckingInput,
    owner: symbols::SymbolHandle,
    requirement: ExpressionHandle,
    intervals: &[validation::ExpressionInterval],
    argument_for: &impl Fn(ExpressionHandle) -> Option<ExpressionHandle>,
    description: &str,
) -> Option<Vec<checked_trees::ConstantRecipeObligation>> {
    use checked_trees::{ConstantRecipeObligation, ConstantRecipeRequirement};
    use numerics::bignum::BigInt;
    if matches!(
        program.expression_table.expression(requirement),
        ExpressionNode::Boolean(true)
    ) || proves(program, requirement, intervals, argument_for)
    {
        return Some(Vec::new());
    }
    let binary = program.expression_table.binary_node(requirement)?;
    if binary.operator == BinaryOperator::And {
        let mut left = recipe_requirements(
            program,
            owner,
            binary.left,
            intervals,
            argument_for,
            description,
        )?;
        left.extend(recipe_requirements(
            program,
            owner,
            binary.right,
            intervals,
            argument_for,
            description,
        )?);
        return Some(left);
    }
    let right_recipe = program.constant_recipe_for_contract_expression(binary.right);
    let left_recipe = program.constant_recipe_for_contract_expression(binary.left);
    let (recipe, subject, operator) = if right_recipe.is_valid() {
        (right_recipe, binary.left, binary.operator)
    } else if left_recipe.is_valid() {
        (left_recipe, binary.right, binary.operator.mirrored())
    } else {
        return None;
    };
    let interval = if let Some(argument) = argument_for(subject) {
        validation::expression_interval(intervals, argument)?
    } else {
        return None;
    };
    let relation = match operator {
        BinaryOperator::Less => ConstantRecipeRequirement::AtLeast(
            BigInt::from_i64(interval.high?).add(&BigInt::from_i64(1)),
        ),
        BinaryOperator::LessOrEqual => {
            ConstantRecipeRequirement::AtLeast(BigInt::from_i64(interval.high?))
        }
        BinaryOperator::Greater => ConstantRecipeRequirement::AtMost(
            BigInt::from_i64(interval.low?).sub(&BigInt::from_i64(1)),
        ),
        BinaryOperator::GreaterOrEqual => {
            ConstantRecipeRequirement::AtMost(BigInt::from_i64(interval.low?))
        }
        BinaryOperator::Equal => {
            return Some(vec![
                ConstantRecipeObligation::retain(
                    owner,
                    recipe,
                    ConstantRecipeRequirement::AtLeast(BigInt::from_i64(interval.high?)),
                    program.expression_table.source_span(requirement),
                    description.to_owned(),
                ),
                ConstantRecipeObligation::retain(
                    owner,
                    recipe,
                    ConstantRecipeRequirement::AtMost(BigInt::from_i64(interval.low?)),
                    program.expression_table.source_span(requirement),
                    description.to_owned(),
                ),
            ]);
        }
        _ => return None,
    };
    Some(vec![ConstantRecipeObligation::retain(
        owner,
        recipe,
        relation,
        program.expression_table.source_span(requirement),
        description.to_owned(),
    )])
}

//! A value dispatch checks its operands and joins the possible arm results.
//!
//! The result type comes from the ordinary type join, not the first convenient
//! arm or the consuming operation. Ranges join by union; an arm's stronger
//! facts stay inside that arm. This is the same analysis for a dispatch used
//! as an initializer, call argument, operand, or result.

use super::{Analysis, ExpressionWalk, NEUTRAL};
use crate::proof_contracts::arithmetic_domains::{
    ArithmeticDomain, Diagnostic, ExpressionHandle, Interval, dispatch_arm_environments,
};
use typed_trees::expression::{MatchPattern, TableMatchExpression};

pub(super) fn analyze(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
    dispatch: &TableMatchExpression,
    diagnostics: &mut Vec<Diagnostic>,
    decisions: &mut super::ArithmeticDecisions,
) -> Analysis {
    let program = walk.program;
    let operand_walk = walk.with_destination(None, ArithmeticDomain::Exact);
    operand_walk.analyze(dispatch.subject, diagnostics, decisions);
    let result_type = walk.result_type(expression);
    let primitive = result_type.and_then(|reference| program.primitive_type_reference(reference));
    let domain =
        result_type.map(|reference| program.arithmetic_domain_for_type_reference(reference));
    let result_walk = walk.with_destination(
        primitive.or(walk.target_primitive),
        domain.unwrap_or(walk.target_domain),
    );
    let mut interval: Option<Interval> = None;
    let mut recipe: Option<checked_trees::IntegerRangeRecipe> = None;

    let environments = walk.machine.map_or_else(
        || vec![walk.environment.clone(); program.expression_table.match_arms(dispatch.arms).len()],
        |machine| {
            dispatch_arm_environments(
                program,
                machine,
                walk.state,
                expression,
                dispatch,
                walk.environment,
            )
        },
    );
    for (arm, selected) in program
        .expression_table
        .match_arms(dispatch.arms)
        .iter()
        .zip(&environments)
    {
        if let MatchPattern::Value(pattern) = arm.pattern {
            operand_walk.analyze(pattern, diagnostics, decisions);
        }
        let arm_walk = ExpressionWalk {
            environment: selected,
            ..result_walk
        };
        let arm_value = arm_walk.analyze(arm.value, diagnostics, decisions);
        let arm_recipe = arm_value.range_recipe(program, walk.lexical_owner, arm.value);
        recipe = if interval.is_none() {
            arm_recipe
        } else {
            recipe
                .zip(arm_recipe)
                .map(|(left, right)| left.union(&right))
        };
        interval = Some(interval.map_or(arm_value.interval, |previous| {
            previous.union(arm_value.interval)
        }));
    }
    Analysis {
        recipe: recipe.filter(checked_trees::IntegerRangeRecipe::is_target_dependent),
        domain,
        primitive,
        interval: interval.unwrap_or(NEUTRAL.interval),
    }
}

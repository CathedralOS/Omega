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
use crate::value_custody::expression_types::expression_result_type_reference;
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
    let result_type = walk.state.and_then(|state| {
        expression_result_type_reference(program, walk.machine, state, expression)
    });
    let primitive = result_type.and_then(|reference| program.primitive_type_reference(reference));
    let domain =
        result_type.map(|reference| program.arithmetic_domain_for_type_reference(reference));
    let result_walk = walk.with_destination(
        primitive.or(walk.target_primitive),
        domain.unwrap_or(walk.target_domain),
    );
    let mut interval: Option<Interval> = None;

    let environments = dispatch_arm_environments(
        program,
        walk.machine,
        walk.state,
        expression,
        dispatch,
        walk.environment,
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
        interval = Some(interval.map_or(arm_value.interval, |previous| {
            previous.union(arm_value.interval)
        }));
    }
    Analysis {
        domain,
        primitive,
        interval: interval.unwrap_or(NEUTRAL.interval),
    }
}

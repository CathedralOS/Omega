//! A value dispatch checks its operands and joins the possible arm results.
//!
//! The result type comes from the ordinary type join, not the first convenient
//! arm or the consuming operation. Ranges join by union; an arm's stronger
//! facts stay inside that arm. This is the same analysis for a dispatch used
//! as an initializer, call argument, operand, or result.

use super::{Analysis, ExpressionWalk, NEUTRAL};
use crate::machine_calls::calls::{CallFrameResolver, expression_has_calls};
use crate::proof_contracts::arithmetic_domains::{
    ArithmeticDomain, Diagnostic, ExpressionHandle, Interval, guard_narrowing,
};
use crate::value_custody::expression_types::expression_result_type_reference;
use typed_trees::expression::{
    BinaryOperator, MatchPattern, TableBinaryExpression, TableMatchExpression,
};

pub(super) fn analyze(
    walk: &ExpressionWalk,
    expression: ExpressionHandle,
    dispatch: &TableMatchExpression,
    diagnostics: &mut Vec<Diagnostic>,
) -> Analysis {
    let program = walk.program;
    let operand_walk = walk.with_destination(None, ArithmeticDomain::Exact);
    operand_walk.analyze(dispatch.subject, diagnostics);
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
    let mut remaining = walk.environment.clone();
    let mut interval: Option<Interval> = None;

    // Statement validation has already crossed nested call frames, but branch
    // conditions below introduce new facts about the saved subject. Those facts
    // cannot describe a mutable place after a pattern or arm changes it. Reuse
    // the call-effect owner instead of assuming a guard's spelling stays live.
    let writes = if expression_has_calls(program, expression) {
        CallFrameResolver::new(program)
            .and_then(|frames| frames.expression_may_write_paths(walk.machine, expression))
    } else {
        Some(Vec::new())
    };
    for arm in program.expression_table.match_arms(dispatch.arms) {
        let mut selected = remaining.clone();
        if let MatchPattern::Value(pattern) = arm.pattern {
            operand_walk.analyze(pattern, diagnostics);
            let comparison = TableBinaryExpression {
                left: dispatch.subject,
                operator: BinaryOperator::Equal,
                right: pattern,
            };
            guard_narrowing::narrow_environment_by_comparison(
                program,
                walk.machine,
                walk.state,
                &mut selected,
                expression,
                &comparison,
                true,
            );
            guard_narrowing::narrow_environment_by_comparison(
                program,
                walk.machine,
                walk.state,
                &mut remaining,
                expression,
                &comparison,
                false,
            );
        }
        match &writes {
            Some(writes) => selected.invalidate_written_paths(writes),
            None => selected.invalidate_unknown_writes(program, walk.state),
        }
        let arm_walk = ExpressionWalk {
            environment: &selected,
            ..result_walk
        };
        let arm_value = arm_walk.analyze(arm.value, diagnostics);
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

use crate::EmissionPlanningInput;
use omega_state_graph::RuntimeTransitionTarget;
use omega_state_guards::{StateGuardLowering, lower_guard_conjunction};
use psi_arena::Arena;
use psi_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable,
};

use super::guard_expression_support::expression_guard_can_emit;
use super::semantic_scope::proof_scope_suffix;
use super::{EmissionBlocker, blocker};

pub(super) fn collect_state_guard_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, guard) in input.state_guards.guards.iter() {
        if matches!(
            guard.lowering,
            StateGuardLowering::NoOp
                | StateGuardLowering::CompareStaticValue
                | StateGuardLowering::CompareRuntimeValue
        ) {
            continue;
        }

        let clauses = lower_guard_conjunction(
            input.state_guards,
            input.layouts,
            input.runtime_storage,
            input.receiver_bases,
            input.state_contexts,
            input.entry_key.machine,
            guard.source,
            guard.source.machine,
            guard.source_dispatch_index,
            guard.statement_order,
        );
        if !clauses.is_empty() {
            continue;
        }

        if guard.has_expression
            && expression_guard_can_emit(
                input,
                guard.source,
                guard.source_dispatch_index,
                guard.statement_index,
                &input.state_guards.expressions.to_tree(guard.expression),
            )
        {
            continue;
        }

        if guard.has_expression
            && disjunctive_expression_guard_can_emit(
                input,
                guard.source,
                guard.source_dispatch_index,
                guard.statement_index,
                &input.state_guards.expressions,
                guard.expression,
            )
        {
            continue;
        }

        let machine_name = input
            .control_flow
            .machine_by_symbol(guard.source.machine)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>");
        let state_name = input
            .control_flow
            .state_by_key(guard.source)
            .map(|state| state.name.as_str())
            .unwrap_or("<unknown>");

        blockers.insert(blocker(
            "state guards",
            &format!(
                "#{} {}.{} edge {} -> #{} {} {:?}/{:?} `{}`{} needs runtime guard lowering",
                guard.source_dispatch_index,
                machine_name,
                state_name,
                guard.statement_order,
                guard.target_dispatch_index,
                runtime_transition_target_name(input, &guard.target),
                guard.kind,
                guard.lowering,
                input
                    .state_guards
                    .expressions
                    .display_name(guard.expression),
                proof_scope_suffix(input, guard.source)
            ),
        ));
    }
}

fn disjunctive_expression_guard_can_emit(
    input: &EmissionPlanningInput<'_>,
    source_key: omega_control_flow::StateKey,
    source_dispatch_index: u32,
    statement_index: usize,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    let disjuncts = expression_disjuncts(expressions, expression);
    disjuncts.len() > 1
        && disjuncts.into_iter().all(|disjunct| {
            expression_guard_can_emit(
                input,
                source_key,
                source_dispatch_index,
                statement_index,
                &expressions.to_tree(disjunct),
            )
        })
}

fn expression_disjuncts(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    if !expression.is_valid() {
        return vec![ExpressionHandle::invalid()];
    }

    let ExpressionNode::Binary(binary) = expressions.expression(expression) else {
        return vec![expression];
    };
    if binary.operator != BinaryOperator::Or {
        return vec![expression];
    }

    let mut disjuncts = expression_disjuncts(expressions, binary.left);
    disjuncts.extend(expression_disjuncts(expressions, binary.right));
    disjuncts
}

fn runtime_transition_target_name(
    input: &EmissionPlanningInput<'_>,
    target: &RuntimeTransitionTarget,
) -> String {
    match target {
        RuntimeTransitionTarget::State { key, .. } => input
            .control_flow
            .state_names_by_key(*key)
            .map(|(machine, state)| format!("{machine}.{state}"))
            .unwrap_or_else(|| "<unknown>.<unknown>".to_owned()),
        RuntimeTransitionTarget::Terminal => "terminal".to_owned(),
        RuntimeTransitionTarget::None => "none".to_owned(),
        RuntimeTransitionTarget::Unknown => "unknown".to_owned(),
    }
}

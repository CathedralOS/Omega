//! Ordered authored call occurrences for source preparation. This traversal
//! records occurrences and conditional ancestry; it never proves a call.

use crate::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression},
};

pub trait ExpressionCalls<'program>: Sized {
    type Site;
    fn source(&self) -> &'program TypedTrees;
    fn named_call_is_selected(
        &self,
        expression: ExpressionHandle,
        call: &'program TableCallExpression,
    ) -> bool;
    fn visit_call(
        &mut self,
        expression: ExpressionHandle,
        call: &'program TableCallExpression,
    ) -> Option<Self::Site>;
    fn selected(
        &mut self,
        visit: impl FnOnce(&mut Self) -> Option<Self::Site>,
    ) -> Option<Self::Site>;
}

pub fn find_in_expression<'program, Calls: ExpressionCalls<'program>>(
    traversal: &mut Calls,
    expression: ExpressionHandle,
) -> Option<Calls::Site> {
    find(traversal, expression)
}

fn find<'program, Calls: ExpressionCalls<'program>>(
    traversal: &mut Calls,
    expression: ExpressionHandle,
) -> Option<Calls::Site> {
    let program = traversal.source();
    if !program.expression_table.expression_is_valid(expression)
        || program.static_type_context_is_unreachable(expression)
    {
        return None;
    }
    let node = program.expression_table.expression(expression);
    match node {
        ExpressionNode::Match(dispatch) => {
            if let Some(site) = find(traversal, dispatch.subject) {
                return Some(site);
            }
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                    if let Some(site) = traversal.selected(|traversal| find(traversal, pattern)) {
                        return Some(site);
                    }
                }
                if let Some(site) = traversal.selected(|traversal| find(traversal, arm.value)) {
                    return Some(site);
                }
            }
            None
        }
        ExpressionNode::Binary(binary) => {
            if let Some(site) = find(traversal, binary.left) {
                return Some(site);
            }
            if matches!(
                (binary.operator, program.static_boolean_value(binary.left)),
                (BinaryOperator::And, Some(false)) | (BinaryOperator::Or, Some(true))
            ) {
                return None;
            }
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                traversal.selected(|traversal| find(traversal, binary.right))
            } else {
                find(traversal, binary.right)
            }
        }
        ExpressionNode::Call(call) => {
            if (call.runtime_call.is_some() || traversal.named_call_is_selected(expression, call))
                && let Some(site) = traversal.visit_call(expression, call)
            {
                return Some(site);
            }
            let mut children = Vec::new();
            program.expression_children(node, |child| children.push(child));
            for child in children {
                if let Some(site) = find(traversal, child) {
                    return Some(site);
                }
            }
            None
        }
        ExpressionNode::Atomic(atomic) => {
            let children = match program.atomic_once_operands(atomic) {
                Some(carrier) => std::iter::once(carrier.place)
                    .chain(carrier.operands)
                    .collect(),
                None => vec![atomic.value],
            };
            for child in children {
                if let Some(site) = find(traversal, child) {
                    return Some(site);
                }
            }
            None
        }
        _ => {
            let mut children = Vec::new();
            program.expression_children(node, |child| children.push(child));
            for child in children {
                if let Some(site) = find(traversal, child) {
                    return Some(site);
                }
            }
            None
        }
    }
}

pub trait StatementCalls<'program>: ExpressionCalls<'program> {
    fn named_statement_call_is_selected(&self, call: &'program crate::statement::TableCall)
    -> bool;
    fn visit_statement_call(
        &mut self,
        call: &'program crate::statement::TableCall,
    ) -> Option<Self::Site>;
    fn visit_transition_call(
        &mut self,
        target: &'program crate::statement::TransitionTargetNode,
    ) -> Option<Self::Site>;
    fn selected_target(
        &mut self,
        target: crate::statement::TransitionTargetHandle,
        visit: impl FnOnce(&mut Self) -> Option<Self::Site>,
    ) -> Option<Self::Site>;
}

pub fn find_in_statement<'program, Calls: StatementCalls<'program>>(
    traversal: &mut Calls,
    statement: &'program crate::statement::StatementNode,
) -> Option<Calls::Site> {
    use crate::statement::{StatementNode, TransitionGuardNode};
    let program = traversal.source();
    match statement {
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => None,
        StatementNode::Assignment(assignment) => {
            find(traversal, assignment.target).or_else(|| find(traversal, assignment.value))
        }
        StatementNode::Call(call) => {
            if (call.runtime_call.is_some() || traversal.named_statement_call_is_selected(call))
                && let Some(site) = traversal.visit_statement_call(call)
            {
                return Some(site);
            }
            if let Some(runtime) = call.runtime_call
                && let Some(site) = find(traversal, runtime.callee)
            {
                return Some(site);
            }
            for argument in program.statement_table.expression_handles(call.arguments) {
                if let Some(site) = find(traversal, *argument) {
                    return Some(site);
                }
            }
            None
        }
        StatementNode::Expression(expression) => find(traversal, *expression),
        StatementNode::LocalData(local) => find(traversal, local.initial_value),
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard
                && let Some(site) = find(traversal, guard)
            {
                return Some(site);
            }
            if let Some(site) = traversal.selected_target(transition.target, |traversal| {
                find_in_transition(traversal, transition.target)
            }) {
                return Some(site);
            }
            if transition.continuation.is_valid() {
                traversal.selected_target(transition.continuation, |traversal| {
                    find_in_transition(traversal, transition.continuation)
                })
            } else {
                None
            }
        }
    }
}

pub fn find_in_transition<'program, Calls: StatementCalls<'program>>(
    traversal: &mut Calls,
    target: crate::statement::TransitionTargetHandle,
) -> Option<Calls::Site> {
    use crate::statement::TransitionTargetNode;
    let program = traversal.source();
    if !target.is_valid() {
        return None;
    }
    let node = program.statement_table.transition_target(target);
    match node {
        TransitionTargetNode::Named { arguments, .. } => {
            if let Some(site) = traversal.visit_transition_call(node) {
                return Some(site);
            }
            for argument in program.statement_table.expression_handles(*arguments) {
                if let Some(site) = find(traversal, *argument) {
                    return Some(site);
                }
            }
            None
        }
        TransitionTargetNode::Value(expression) => find(traversal, *expression),
        TransitionTargetNode::Terminal => None,
    }
}

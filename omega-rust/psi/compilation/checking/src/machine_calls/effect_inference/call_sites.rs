//! The single executable call-site walk shared by effect inference.
//!
//! Operational may-suspend/blocking summaries and synchronous-invocation
//! inference consume the same authored call sites in the same
//! `(state, statement, ordinal-within-statement)` order that borrow, flow,
//! contracts and diagnostics join. Each pass supplies only what it records at
//! a site, so a change to authored statement or transition shapes cannot
//! reorder one pass against the other.
//!
//! A named transfer reserves its parent ordinal before its argument calls,
//! including an internal transfer that resolves to no executable target, and
//! every reached call reserves its ordinal even when the pass records nothing
//! for it (a proof-embedding value term).

use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TableCall, TableNamePath, TransitionGuardNode, TransitionTargetNode,
};

/// What one inference pass records at each call site the walk reaches.
pub(super) trait CallSiteVisitor {
    fn statement_call(&mut self, call: &TableCall, statement_index: usize, ordinal: usize);

    fn expression_call(
        &mut self,
        call: &TableCallExpression,
        statement_index: usize,
        ordinal: usize,
    );

    /// A named transition transfer whose target resolves to a call symbol.
    /// The authored `arguments` stay available for the pass that resolves
    /// invocation origins; the walk visits their calls after this site.
    fn named_transfer(
        &mut self,
        target: SymbolHandle,
        path: &TableNamePath,
        arguments: HandleSpan<ExpressionHandle>,
        static_machine_parameter: SymbolHandle,
        acknowledgement: language_semantics::CallOperationalAcknowledgement,
        statement_index: usize,
        ordinal: usize,
    );
}

/// Walk one state's authored statements in order. Call-site identity is
/// `(state, statement, ordinal-within-statement)`, shared with borrow, flow,
/// contracts and diagnostics.
pub(super) fn visit_state_calls(
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
    state: &State,
    visitor: &mut impl CallSiteVisitor,
) {
    for (statement_index, statement) in program
        .statement_table
        .indexed_statements(state.statement_nodes)
    {
        let mut ordinal = 0usize;
        visit_statement(
            program,
            machine,
            statement,
            statement_index,
            &mut ordinal,
            visitor,
        );
    }
}

fn visit_statement(
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
    statement: &StatementNode,
    statement_index: usize,
    ordinal: &mut usize,
    visitor: &mut impl CallSiteVisitor,
) {
    match statement {
        // Assembly contract facts are proof obligations, never runtime calls.
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            visit_expression(
                program,
                assignment.target,
                statement_index,
                ordinal,
                visitor,
            );
            visit_expression(program, assignment.value, statement_index, ordinal, visitor);
        }
        StatementNode::Call(call) => {
            visitor.statement_call(call, statement_index, *ordinal);
            reserve_ordinal(ordinal);
            for argument in program.statement_table.expression_handles(call.arguments) {
                visit_expression(program, *argument, statement_index, ordinal, visitor);
            }
        }
        StatementNode::Expression(expression) => {
            visit_expression(program, *expression, statement_index, ordinal, visitor);
        }
        StatementNode::LocalData(local_data) => {
            if local_data.initial_value.is_valid() {
                visit_expression(
                    program,
                    local_data.initial_value,
                    statement_index,
                    ordinal,
                    visitor,
                );
            }
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                visit_expression(program, guard, statement_index, ordinal, visitor);
            }
            visit_transition_target(
                program,
                machine,
                transition.target,
                statement_index,
                ordinal,
                visitor,
            );
            if transition.continuation.is_valid() {
                visit_transition_target(
                    program,
                    machine,
                    transition.continuation,
                    statement_index,
                    ordinal,
                    visitor,
                );
            }
        }
    }
}

fn visit_transition_target(
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
    target: typed_trees::statement::TransitionTargetHandle,
    statement_index: usize,
    ordinal: &mut usize,
    visitor: &mut impl CallSiteVisitor,
) {
    if !target.is_valid() {
        return;
    }
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path,
            arguments,
            static_machine_parameter,
            operational_acknowledgement,
            ..
        } => {
            let target = crate::declarations::transitions::named_transition_call_symbol(
                program,
                machine,
                path.symbol,
            );
            if target.is_valid() {
                visitor.named_transfer(
                    target,
                    path,
                    *arguments,
                    *static_machine_parameter,
                    *operational_acknowledgement,
                    statement_index,
                    *ordinal,
                );
            }
            reserve_ordinal(ordinal);
            for argument in program.statement_table.expression_handles(*arguments) {
                visit_expression(program, *argument, statement_index, ordinal, visitor);
            }
        }
        TransitionTargetNode::Value(expression) => {
            visit_expression(program, *expression, statement_index, ordinal, visitor);
        }
        TransitionTargetNode::Terminal => {}
    }
}

fn visit_expression(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    statement_index: usize,
    ordinal: &mut usize,
    visitor: &mut impl CallSiteVisitor,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            let inputs =
                crate::value_custody::atomic_operations::atomic_execution_children(program, atomic);
            for input in inputs {
                visit_expression(program, input, statement_index, ordinal, visitor);
            }
        }
        ExpressionNode::Call(call) => {
            visitor.expression_call(call, statement_index, *ordinal);
            reserve_ordinal(ordinal);
            visit_expression(program, call.receiver, statement_index, ordinal, visitor);
            for argument in program.expression_table.expression_handles(call.arguments) {
                visit_expression(program, *argument, statement_index, ordinal, visitor);
            }
        }
        other => crate::expression_traversal::children(program, other, |child| {
            visit_expression(program, child, statement_index, ordinal, visitor);
        }),
    }
}

fn reserve_ordinal(ordinal: &mut usize) {
    *ordinal = ordinal.checked_add(1).expect("call ordinal overflow");
}

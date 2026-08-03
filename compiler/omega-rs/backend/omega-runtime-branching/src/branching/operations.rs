use crate::RuntimeBranchingContext;
use omega_control_flow::{OperationKind, StateKey};
use omega_state_calls::{StateCall, StateCallRole};
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::expression::ExpressionTable;

use super::lookups::{host_call_for_statement, mutation_for_statement, state_call_for_operation};
use super::{
    RuntimeBranchPreludeOperation, RuntimeBranchPreludeOperationKind, RuntimeLeafBranchOperation,
    RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchOperation,
    RuntimeStraightLineBranchOperationKind,
};

/// Which of the callee's statements a branch prelude expansion carries.
///
/// The runtime-bodies SPLICE executes a non-guard callee's mutations, host calls,
/// and nested calls (flattening the nested calls into the caller's body); the
/// prelude must not re-emit those or the callee's side effects run twice per
/// evaluation. The splice does NOT cover the callee's LOCAL initializers (e.g.
/// `let cells = self.bag.cells.as_mut_slice();`), so every executing prelude
/// keeps LocalData. Guard-role calls use the prelude as their ONLY executor
/// (their splice is skipped so repeated transition subjects can dedupe), so
/// their preludes carry everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreludeStatementFilter {
    /// Guard-role executor prelude: every callee statement.
    All,
    /// Non-guard prelude: local initializers only; the splice executes the rest.
    LocalDataOnly,
    /// Repeated guard subject: no statements (bindings + value selection only).
    None,
}

pub(super) fn prelude_operations(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    output_operations: &mut Arena<RuntimeBranchPreludeOperation>,
    source_key: StateKey,
    filter: PreludeStatementFilter,
) -> HandleSpan<RuntimeBranchPreludeOperation> {
    if filter == PreludeStatementFilter::None {
        return HandleSpan::empty();
    }
    let Some(state) = context.control_flow.state_by_key(source_key) else {
        return HandleSpan::empty();
    };
    let Some(operations) = context.control_flow.operations.span(state.operations) else {
        return HandleSpan::empty();
    };

    let kept: Vec<RuntimeBranchPreludeOperation> = operations
        .iter()
        .map(|operation| RuntimeBranchPreludeOperation {
            source_key,
            statement_index: operation.statement_index,
            kind: prelude_operation_kind(
                context,
                expressions,
                source_key,
                operation.statement_index,
                &operation.kind,
            ),
        })
        .filter(|operation| {
            // Non-guard preludes carry ONLY call-free local initializers: the
            // splice executes the callee's mutations, host calls, AND state
            // calls (a `let x = self.f(...)` statement classifies as StateCall
            // here, and the prelude's StateCall arm re-emits the callee's
            // nested expansions -- one extra leaf run per statement chain, the
            // dungeon's non-guard RNG over-draw).
            filter == PreludeStatementFilter::All
                || matches!(
                    operation.kind,
                    RuntimeBranchPreludeOperationKind::LocalData
                        | RuntimeBranchPreludeOperationKind::Other
                )
        })
        .collect();
    output_operations.insert_many(kept)
}

// SUB-STATE bodies flattened by the leaf-expansion walk have no other
// emission route: a non-leaf sub-state (one with its own nested transition)
// gets neither a prelude (that covers only the callee ROOT) nor an
// InlineLeaf expansion (those are for transition-free leaves), so its
// arm-local `let` initializers vanish natively (the multiarm texteq
// divergence). This collects EXACTLY those: call-free LocalData ops. The
// two exclusions are load-bearing:
//  * call-carrying statements re-emit the callee's nested expansions once
//    per arm (the single-execution + dungeon 10-red regression), and
//  * callers must SKIP the callee root (`branch_key == state_call.target_key`)
//    or root-level captured-before-mutation locals re-initialize at
//    terminal time (the trailing-local-return reset-before-return 7-red
//    regression).
// Known remaining gap (no observed shape yet): a chain of TWO nested
// sub-states only collects the innermost body.
pub(super) fn leaf_local_initializer_operations(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    output_operations: &mut Arena<RuntimeLeafBranchOperation>,
    source_key: StateKey,
) -> HandleSpan<RuntimeLeafBranchOperation> {
    let Some(state) = context.control_flow.state_by_key(source_key) else {
        return HandleSpan::empty();
    };
    let Some(operations) = context.control_flow.operations.span(state.operations) else {
        return HandleSpan::empty();
    };

    let kept: Vec<RuntimeLeafBranchOperation> = operations
        .iter()
        .filter(|operation| {
            matches!(operation.kind, omega_control_flow::OperationKind::LocalData)
                && state_call_for_operation(context, source_key, operation.statement_index)
                    .is_none()
                && host_call_for_statement(context, source_key, operation.statement_index).is_none()
        })
        .map(|operation| RuntimeLeafBranchOperation {
            source_key,
            statement_index: operation.statement_index,
            kind: leaf_operation_kind(context, expressions, source_key, operation.statement_index),
        })
        .collect();
    output_operations.insert_many(kept)
}

pub(super) fn leaf_operations(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    output_operations: &mut Arena<RuntimeLeafBranchOperation>,
    source_key: StateKey,
) -> HandleSpan<RuntimeLeafBranchOperation> {
    let Some(state) = context.control_flow.state_by_key(source_key) else {
        return HandleSpan::empty();
    };
    let Some(operations) = context.control_flow.operations.span(state.operations) else {
        return HandleSpan::empty();
    };

    output_operations.insert_many(
        operations
            .iter()
            .map(|operation| RuntimeLeafBranchOperation {
                source_key,
                statement_index: operation.statement_index,
                kind: leaf_operation_kind(
                    context,
                    expressions,
                    source_key,
                    operation.statement_index,
                ),
            }),
    )
}

pub(super) fn straight_line_operations(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    output_operations: &mut Arena<RuntimeStraightLineBranchOperation>,
    source_key: StateKey,
) -> HandleSpan<RuntimeStraightLineBranchOperation> {
    let Some(state) = context.control_flow.state_by_key(source_key) else {
        return HandleSpan::empty();
    };
    let Some(operations) = context.control_flow.operations.span(state.operations) else {
        return HandleSpan::empty();
    };

    let mut branch_operations = Vec::with_capacity(operations.len().saturating_mul(2));
    for operation in operations {
        for state_call in
            state_calls_for_straight_line_operation(context, source_key, operation.statement_index)
        {
            branch_operations.push(RuntimeStraightLineBranchOperation {
                source_key,
                statement_index: operation.statement_index,
                kind: RuntimeStraightLineBranchOperationKind::StateCall {
                    role: state_call.role,
                    call_ordinal: state_call.call_ordinal,
                    target_key: state_call.target_key,
                    argument_count: state_call.argument_count,
                    lowering: state_call.lowering,
                },
            });
        }

        branch_operations.push(RuntimeStraightLineBranchOperation {
            source_key,
            statement_index: operation.statement_index,
            kind: straight_line_operation_kind(
                context,
                expressions,
                source_key,
                operation.statement_index,
                &operation.kind,
            ),
        });
    }

    output_operations.insert_many(branch_operations)
}

fn state_calls_for_straight_line_operation<'plan>(
    context: &'plan RuntimeBranchingContext<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> impl Iterator<Item = &'plan StateCall> {
    context
        .state_calls
        .calls_for_statement(source_key, statement_index)
        .filter(|state_call| {
            matches!(
                state_call.role,
                StateCallRole::Statement
                    | StateCallRole::AssignmentValue
                    | StateCallRole::CallArgument
            )
        })
}

fn prelude_operation_kind(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
    operation_kind: &OperationKind,
) -> RuntimeBranchPreludeOperationKind {
    if host_call_for_statement(context, source_key, statement_index).is_some() {
        return RuntimeBranchPreludeOperationKind::HostCall;
    }

    if let Some(mutation) = mutation_for_statement(context, source_key, statement_index) {
        return RuntimeBranchPreludeOperationKind::Mutation {
            mutation_kind: mutation.mutation_kind,
            lowering: mutation.lowering,
            target: expressions.copy_from(&context.state_storage.expressions, mutation.target),
            value: expressions.copy_from(&context.state_storage.expressions, mutation.value),
        };
    }

    if let Some(state_call) = state_call_for_operation(context, source_key, statement_index) {
        return RuntimeBranchPreludeOperationKind::StateCall {
            role: state_call.role,
            call_ordinal: state_call.call_ordinal,
            target_key: state_call.target_key,
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        };
    }

    if matches!(operation_kind, OperationKind::LocalData) {
        return RuntimeBranchPreludeOperationKind::LocalData;
    }

    RuntimeBranchPreludeOperationKind::Other
}

fn leaf_operation_kind(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
) -> RuntimeLeafBranchOperationKind {
    if host_call_for_statement(context, source_key, statement_index).is_some() {
        return RuntimeLeafBranchOperationKind::HostCall;
    }

    if let Some(mutation) = mutation_for_statement(context, source_key, statement_index) {
        return RuntimeLeafBranchOperationKind::Mutation {
            mutation_kind: mutation.mutation_kind,
            lowering: mutation.lowering,
            target: expressions.copy_from(&context.state_storage.expressions, mutation.target),
            value: expressions.copy_from(&context.state_storage.expressions, mutation.value),
        };
    }

    RuntimeLeafBranchOperationKind::Other
}

fn straight_line_operation_kind(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
    operation_kind: &OperationKind,
) -> RuntimeStraightLineBranchOperationKind {
    if host_call_for_statement(context, source_key, statement_index).is_some() {
        return RuntimeStraightLineBranchOperationKind::HostCall;
    }

    if let Some(mutation) = mutation_for_statement(context, source_key, statement_index) {
        return RuntimeStraightLineBranchOperationKind::Mutation {
            mutation_kind: mutation.mutation_kind,
            lowering: mutation.lowering,
            target: expressions.copy_from(&context.state_storage.expressions, mutation.target),
            value: expressions.copy_from(&context.state_storage.expressions, mutation.value),
        };
    }

    if matches!(operation_kind, OperationKind::LocalData) {
        return RuntimeStraightLineBranchOperationKind::LocalData;
    }

    RuntimeStraightLineBranchOperationKind::Other
}

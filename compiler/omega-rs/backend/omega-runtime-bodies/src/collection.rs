use super::context::RuntimeDispatchBodyContext;
use super::lookups::{
    host_call_for_statement, local_storage_for_statement, mutation_for_statement,
    state_call_for_statement, state_has_no_transitions, state_operations,
};
use super::model::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_control_flow::{OperationExpressionRefs, OperationKind, StateKey};
use omega_state_calls::{StateCall, StateCallLowering, StateCallRole};
use omega_state_dispatch::DispatchState;
use omega_state_graph::RuntimeTransitionTarget;
use psi_arena::Arena;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_checked_trees::name::Identifier;
use psi_checked_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use psi_checked_trees::types::TypeReferenceTable;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CollectedRuntimeDispatchBody {
    pub key: StateKey,
    pub dispatch_index: u32,
    pub expressions: ExpressionTable,
    pub invariant_names: Arena<Identifier>,
    pub operations: Arena<RuntimeDispatchBodyOperation>,
    pub type_references: TypeReferenceTable,
}

const INLINE_BODY_VISITING_COUNT: usize = 16;

struct BodyVisitingStates {
    inline: [Option<StateKey>; INLINE_BODY_VISITING_COUNT],
    len: usize,
    overflow: Vec<StateKey>,
}

impl BodyVisitingStates {
    fn with_capacity(state_capacity: usize) -> Self {
        Self {
            inline: [None; INLINE_BODY_VISITING_COUNT],
            len: 0,
            overflow: Vec::with_capacity(state_capacity.saturating_sub(INLINE_BODY_VISITING_COUNT)),
        }
    }

    fn contains(&self, key: StateKey) -> bool {
        self.inline
            .iter()
            .take(self.len.min(INLINE_BODY_VISITING_COUNT))
            .flatten()
            .any(|candidate| *candidate == key)
            || self.overflow.contains(&key)
    }

    fn push(&mut self, key: StateKey) {
        if self.len < INLINE_BODY_VISITING_COUNT {
            self.inline[self.len] = Some(key);
        } else {
            self.overflow.push(key);
        }

        self.len += 1;
    }

    fn pop(&mut self) {
        if self.len == 0 {
            return;
        }

        self.len -= 1;
        if self.len < INLINE_BODY_VISITING_COUNT {
            self.inline[self.len] = None;
        } else {
            self.overflow.pop();
        }
    }
}

pub(super) fn build_dispatch_body(
    context: &RuntimeDispatchBodyContext,
    dispatch_state: &DispatchState,
) -> CollectedRuntimeDispatchBody {
    let operation_capacity = estimated_body_operation_capacity(context, dispatch_state.key);
    let mut operations = Arena::with_capacity(operation_capacity);
    let mut expressions = ExpressionTable::with_expression_capacity(operation_capacity);
    let mut invariant_names = Arena::with_capacity(estimated_body_invariant_name_capacity(
        context,
        dispatch_state.key,
    ));
    let mut type_references = TypeReferenceTable::new();
    // A dispatch state may be one SEGMENT of a control-flow state that was split
    // at its dispatched-call boundaries (segment_index > 0). Its body is the slice
    // of the control-flow state's operations owned by this segment; the operations
    // keep the control-flow (segment 0) source key so downstream host-call /
    // mutation / slot lookups still resolve.
    let control_key = StateKey {
        segment_index: 0,
        ..dispatch_state.key
    };
    let segment = segment_filter(context, dispatch_state.key);
    append_state_body_operations(
        context,
        control_key,
        segment,
        &mut operations,
        &mut expressions,
        &mut invariant_names,
        &mut type_references,
        &mut BodyVisitingStates::with_capacity(context.control_flow.states.len()),
    );

    CollectedRuntimeDispatchBody {
        key: dispatch_state.key,
        dispatch_index: dispatch_state.dispatch_index,
        expressions,
        invariant_names,
        operations,
        type_references,
    }
}

fn estimated_body_operation_capacity(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
) -> usize {
    state_operations(context, state_key).map_or(0, <[omega_control_flow::Operation]>::len)
        + context
            .state_calls
            .calls
            .iter()
            .filter(|(_, state_call)| state_call.source_key == state_key)
            .count()
}

fn estimated_body_invariant_name_capacity(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
) -> usize {
    context
        .state_storage
        .locals
        .iter()
        .filter(|(_, local)| local.source_key == state_key)
        .map(|(_, local)| local.invariant_names.len())
        .sum()
}

/// The slice of a control-flow state's operations owned by one segment, plus
/// whether it is the tail segment (the one that runs the state's transition).
/// `None` means the state has no dispatched calls and is not segmented.
#[derive(Debug, Clone, Copy)]
struct SegmentSlice {
    /// Inclusive lower and upper statement-index bounds for this segment's ops.
    low: usize,
    high: usize,
    /// Two dispatched calls can occupy the same statement. The segment between
    /// them has no caller-owned operation to run; it exists only to dispatch
    /// the next call after the first returns.
    is_empty: bool,
    is_tail: bool,
    /// The dispatched call immediately before this segment, if any.
    previous_boundary: Option<(usize, usize, StateCallRole)>,
    /// The dispatched call that ends this segment, if any.
    boundary: Option<(usize, usize, StateCallRole)>,
}

/// Compute which operations of `segment_key`'s control-flow state belong to this
/// segment. A state is split at each DISPATCHED call: segment `i` owns the ops up
/// to and including call `i` (call `i` itself is dispatched, not emitted), the
/// tail segment owns everything after the last call (including its transition).
fn segment_filter(
    context: &RuntimeDispatchBodyContext,
    segment_key: StateKey,
) -> Option<SegmentSlice> {
    let control_key = StateKey {
        segment_index: 0,
        ..segment_key
    };
    let mut boundaries: Vec<(usize, usize, StateCallRole)> = context
        .state_calls
        .calls
        .iter()
        .map(|(_, state_call)| state_call)
        .filter(|state_call| state_call.source_key == control_key)
        .filter(|state_call| state_call_splits_runtime_body(context, state_call))
        .map(|state_call| {
            (
                state_call.statement_index,
                state_call.call_ordinal,
                state_call.role,
            )
        })
        .collect();
    if boundaries.is_empty() {
        return None;
    }
    boundaries.sort_unstable_by_key(|(statement_index, call_ordinal, _)| {
        (*statement_index, *call_ordinal)
    });

    let count = boundaries.len();
    let segment = segment_key.segment_index;
    if segment >= count {
        Some(SegmentSlice {
            // The statement containing the final call executes only after that
            // call returns, in the tail segment.
            low: boundaries[count - 1].0,
            high: usize::MAX,
            is_empty: false,
            is_tail: true,
            previous_boundary: Some(boundaries[count - 1]),
            boundary: None,
        })
    } else {
        let high_exclusive = boundaries[segment].0;
        let low = if segment == 0 {
            0
        } else {
            // The previous call's containing statement may now finish, unless
            // this is another call in that same statement.
            boundaries[segment - 1].0
        };
        Some(SegmentSlice {
            low,
            high: high_exclusive.saturating_sub(1),
            is_empty: high_exclusive == 0 || low >= high_exclusive,
            is_tail: false,
            previous_boundary: segment.checked_sub(1).map(|previous| boundaries[previous]),
            boundary: Some(boundaries[segment]),
        })
    }
}

fn append_state_body_operations(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
    segment: Option<SegmentSlice>,
    operations: &mut Arena<RuntimeDispatchBodyOperation>,
    expressions: &mut ExpressionTable,
    invariant_names: &mut Arena<Identifier>,
    type_references: &mut TypeReferenceTable,
    visiting: &mut BodyVisitingStates,
) {
    if visiting.contains(state_key) {
        return;
    }
    visiting.push(state_key);

    let Some(state_operations) = state_operations(context, state_key) else {
        visiting.pop();
        return;
    };

    for operation in state_operations {
        // A dispatched statement call ends this segment, but calls nested in
        // its receiver/arguments are part of evaluating that statement and
        // must run before the outer dispatch. Statement-only slicing used to
        // drop these operations from the pre-call segment (especially at
        // statement zero) and then emit them in the continuation, after the
        // outer callee had already consumed an uninitialized result slot.
        if let Some(slice) = segment
            && slice
                .boundary
                .is_some_and(|(statement_index, _, _)| statement_index == operation.statement_index)
        {
            append_call_argument_operations_for_segment(
                context,
                state_key,
                operation.statement_index,
                slice,
                operations,
                expressions,
                invariant_names,
                type_references,
                visiting,
            );
            continue;
        }
        // Skip operations outside this segment's statement-index window.
        if let Some(slice) = segment
            && (slice.is_empty
                || operation.statement_index < slice.low
                || operation.statement_index > slice.high)
        {
            continue;
        }
        // An `asm { hlt }` statement lowers to a raw MachineHalt, not a state
        // transition -- classify it before the host-call / state-call checks so
        // its unresolved call record does not become a StateCall body op.
        if let OperationKind::Call {
            target,
            has_receiver: false,
            ..
        } = &operation.kind
            && target.as_str() == "asm#hlt"
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::MachineHalt,
            ));
            continue;
        }
        if let OperationKind::Call {
            target,
            has_receiver: false,
            ..
        } = &operation.kind
            && target.as_str() == "asm#wrmsr"
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::MsrWrite,
            ));
            continue;
        }
        if let OperationKind::Call {
            target,
            has_receiver: false,
            ..
        } = &operation.kind
            && let Some(register) =
                psi_language_core::inline_assembly::AsmControlRegister::from_write_intrinsic_name(
                    target.as_str(),
                )
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::ControlRegisterWrite(register),
            ));
            continue;
        }
        if let OperationKind::Call {
            target,
            has_receiver: false,
            ..
        } = &operation.kind
            && let Some(kind) =
                psi_language_core::inline_assembly::AsmFenceKind::from_intrinsic_name(
                    target.as_str(),
                )
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::MemoryFence(kind),
            ));
            continue;
        }
        if let OperationKind::Call {
            target,
            has_receiver: false,
            ..
        } = &operation.kind
            && let Some(kind) =
                psi_language_core::inline_assembly::AsmInterruptControlKind::from_intrinsic_name(
                    target.as_str(),
                )
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::InterruptControl(kind),
            ));
            continue;
        }
        if let OperationKind::Call {
            target,
            has_receiver: false,
            ..
        } = &operation.kind
            && target.as_str() == "asm#popfq"
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::FlagsRestore,
            ));
            continue;
        }
        // `asm { out <port>, <value> }` -- a Call to `asm#port_out` -- lowers to
        // a raw port write, not a state call.
        if let OperationKind::Call {
            target,
            has_receiver: false,
            ..
        } = &operation.kind
            && target.as_str() == "asm#port_out"
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::PortWrite,
            ));
            continue;
        }
        // `asm { in <dest>, <port> }` desugars to `dest = asm#port_in(port)`, an
        // ASSIGNMENT whose value is the intrinsic call. Classify it as a raw
        // port read BEFORE the mutation/state-call handling, which would
        // otherwise try to lower the unresolved `asm#port_in` call.
        if matches!(operation.kind, OperationKind::Assignment)
            && let Some(mutation) =
                mutation_for_statement(context, state_key, operation.statement_index)
            && let psi_checked_trees::expression::ExpressionNode::Call(call) =
                context.state_storage.expressions.expression(mutation.value)
            && call.target.as_str() == "asm#port_in"
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::PortRead,
            ));
            continue;
        }
        if matches!(operation.kind, OperationKind::Assignment)
            && let Some(mutation) =
                mutation_for_statement(context, state_key, operation.statement_index)
            && let psi_checked_trees::expression::ExpressionNode::Call(call) =
                context.state_storage.expressions.expression(mutation.value)
            && let Some(register) =
                psi_language_core::inline_assembly::AsmControlRegister::from_read_intrinsic_name(
                    call.target.as_str(),
                )
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::ControlRegisterRead(register),
            ));
            continue;
        }
        if matches!(operation.kind, OperationKind::Assignment)
            && let Some(mutation) =
                mutation_for_statement(context, state_key, operation.statement_index)
            && let psi_checked_trees::expression::ExpressionNode::Call(call) =
                context.state_storage.expressions.expression(mutation.value)
            && call.target.as_str() == "asm#rdmsr"
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::MsrRead,
            ));
            continue;
        }
        if matches!(operation.kind, OperationKind::Assignment)
            && let Some(mutation) =
                mutation_for_statement(context, state_key, operation.statement_index)
            && let psi_checked_trees::expression::ExpressionNode::Call(call) =
                context.state_storage.expressions.expression(mutation.value)
            && call.target.as_str() == "asm#pushfq"
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::FlagsSnapshot,
            ));
            continue;
        }
        append_call_argument_operations_for_segment(
            context,
            state_key,
            operation.statement_index,
            segment.unwrap_or(SegmentSlice {
                low: 0,
                high: usize::MAX,
                is_empty: false,
                is_tail: true,
                previous_boundary: None,
                boundary: None,
            }),
            operations,
            expressions,
            invariant_names,
            type_references,
            visiting,
        );

        // A host call owns the statement-position operation, but machine value
        // calls nested in its arguments must execute first and leave their
        // results in the call-argument slots appended above.
        if let Some(host_call) =
            host_call_for_statement(context, state_key, operation.statement_index)
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::HostCall {
                    call_ordinal: host_call.call_ordinal,
                },
            ));
            continue;
        }

        if let Some(state_call) =
            state_call_for_statement(context, state_key, operation.statement_index)
        {
            if state_call_splits_runtime_body(context, state_call) {
                continue;
            }
            append_state_call_body_operation(
                context,
                state_call,
                operations,
                expressions,
                invariant_names,
                type_references,
                visiting,
            );
            continue;
        }

        let mut assignment_value_calls = context
            .state_calls
            .calls_for_statement(state_key, operation.statement_index)
            .filter(|state_call| state_call.role == StateCallRole::AssignmentValue)
            .collect::<Vec<_>>();
        assignment_value_calls.sort_by_key(|state_call| state_call.call_ordinal);
        if let OperationExpressionRefs::Assignment { value, .. }
        | OperationExpressionRefs::Expression(value) = operation.expressions
        {
            assignment_value_calls =
                assignment_value_calls_in_evaluation_order(context, value, assignment_value_calls);
        }
        for state_call in assignment_value_calls {
            // A DISPATCHED value call's result arrives via the dispatch terminal
            // writing the callee's return into the call-result slot; do not also
            // inline-expand it here (that would double-lower and unroll a loop).
            if !state_call_is_dispatched(context, state_call) {
                append_state_call_body_operation(
                    context,
                    state_call,
                    operations,
                    expressions,
                    invariant_names,
                    type_references,
                    visiting,
                );
            }
        }

        if let Some(local_storage) =
            local_storage_for_statement(context, state_key, operation.statement_index)
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::LocalStorage {
                    symbol: local_storage.symbol,
                    name: local_storage.name.clone(),
                    type_symbol: local_storage.type_symbol,
                    type_reference: type_references.copy_from(
                        &context.state_storage.type_references,
                        &context.state_storage.expressions,
                        expressions,
                        local_storage.type_reference,
                    ),
                    invariant_names: invariant_names.insert_many(
                        context
                            .state_storage
                            .invariant_names
                            .span_or_empty(local_storage.invariant_names)
                            .iter()
                            .cloned(),
                    ),
                },
            ));
            continue;
        }

        if let Some(mutation) =
            mutation_for_statement(context, state_key, operation.statement_index)
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::Mutation {
                    mutation_kind: mutation.mutation_kind,
                    lowering: mutation.lowering,
                },
            ));
            continue;
        }

        if !matches!(operation.kind, OperationKind::LocalData) {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::Other,
            ));
        }
    }

    // Transition guards/arguments are evaluated when the state takes its
    // transition, which only the TAIL segment does. Skip them for earlier
    // segments (an unsegmented state -- `None` -- always runs them).
    if segment.is_none_or(|slice| slice.is_tail) {
        for (_, state_call) in context.state_calls.calls.iter() {
            if state_call.source_key == state_key
                && matches!(
                    state_call.role,
                    omega_state_calls::StateCallRole::TransitionArgument
                        | omega_state_calls::StateCallRole::TransitionGuard
                )
            {
                append_state_call_body_operation(
                    context,
                    state_call,
                    operations,
                    expressions,
                    invariant_names,
                    type_references,
                    visiting,
                );
            }
        }
    }

    visiting.pop();
}

#[allow(clippy::too_many_arguments)]
fn append_call_argument_operations_for_segment(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
    statement_index: usize,
    segment: SegmentSlice,
    operations: &mut Arena<RuntimeDispatchBodyOperation>,
    expressions: &mut ExpressionTable,
    invariant_names: &mut Arena<Identifier>,
    type_references: &mut TypeReferenceTable,
    visiting: &mut BodyVisitingStates,
) {
    for state_call in context
        .state_calls
        .calls_for_statement(state_key, statement_index)
        .filter(|state_call| state_call.role == StateCallRole::CallArgument)
    {
        let follows_previous = segment.previous_boundary.is_none_or(
            |(previous_statement, previous_ordinal, previous_role)| {
                statement_index != previous_statement
                    || (previous_role != StateCallRole::Statement
                        && state_call.call_ordinal > previous_ordinal)
            },
        );
        let precedes_boundary =
            segment
                .boundary
                .is_none_or(|(boundary_statement, boundary_ordinal, boundary_role)| {
                    statement_index != boundary_statement
                        || boundary_role == StateCallRole::Statement
                        || state_call.call_ordinal < boundary_ordinal
                });
        if follows_previous && precedes_boundary {
            append_state_call_body_operation(
                context,
                state_call,
                operations,
                expressions,
                invariant_names,
                type_references,
                visiting,
            );
        }
    }
}

/// State-call ordinals are minted in call-preorder (an outer call precedes
/// calls nested in its receiver/arguments), while evaluation must produce an
/// inner result before expanding the outer call that consumes it. Reorder the
/// calls to expression postorder without disturbing left-to-right siblings.
fn assignment_value_calls_in_evaluation_order<'plan>(
    context: &'plan RuntimeDispatchBodyContext,
    root: ExpressionHandle,
    calls: Vec<&'plan StateCall>,
) -> Vec<&'plan StateCall> {
    fn receiver_symbol(
        expressions: &ExpressionTable,
        expression: ExpressionHandle,
    ) -> psi_symbols::SymbolHandle {
        if !expression.is_valid() {
            return psi_symbols::SymbolHandle::invalid();
        }
        match expressions.expression(expression) {
            ExpressionNode::Member(member) => member.member_symbol,
            ExpressionNode::Mutable(inner) => receiver_symbol(expressions, *inner),
            ExpressionNode::Name(path) => path.symbol,
            _ => psi_symbols::SymbolHandle::invalid(),
        }
    }

    fn call_matches(
        context: &RuntimeDispatchBodyContext,
        state_call: &StateCall,
        call: &psi_checked_trees::expression::TableCallExpression,
    ) -> bool {
        let target_matches = state_call.target_key.state == call.target_symbol
            || context
                .control_flow
                .state_by_key(state_call.target_key)
                .is_some_and(|state| state.name == call.target);
        if !target_matches {
            return false;
        }
        let actual_receiver = receiver_symbol(&context.control_flow.expressions, call.receiver);
        !actual_receiver.is_valid()
            || !state_call.receiver_symbol.is_valid()
            || actual_receiver == state_call.receiver_symbol
    }

    fn visit<'plan>(
        context: &RuntimeDispatchBodyContext,
        expression: ExpressionHandle,
        calls: &[&'plan StateCall],
        cursor: &mut usize,
        ordered: &mut Vec<&'plan StateCall>,
    ) {
        let expressions = &context.control_flow.expressions;
        match expressions.expression(expression) {
            ExpressionNode::Atomic(atomic) => visit(context, atomic.value, calls, cursor, ordered),
            ExpressionNode::ArrayLiteral(values) => {
                for value in expressions.expression_handles(*values) {
                    visit(context, *value, calls, cursor, ordered);
                }
            }
            ExpressionNode::Binary(binary) => {
                visit(context, binary.left, calls, cursor, ordered);
                visit(context, binary.right, calls, cursor, ordered);
            }
            ExpressionNode::Call(call) => {
                // Consume this call's preorder record before descending, then
                // append it after its dependencies have been appended.
                let current = calls
                    .get(*cursor)
                    .copied()
                    .filter(|state_call| call_matches(context, state_call, call));
                if current.is_some() {
                    *cursor += 1;
                }
                if call.receiver.is_valid() {
                    visit(context, call.receiver, calls, cursor, ordered);
                }
                for argument in expressions.expression_handles(call.arguments) {
                    visit(context, *argument, calls, cursor, ordered);
                }
                if let Some(current) = current {
                    ordered.push(current);
                }
            }
            ExpressionNode::Cast(cast) => visit(context, cast.value, calls, cursor, ordered),
            ExpressionNode::Indexed(indexed) => {
                visit(context, indexed.collection, calls, cursor, ordered);
                visit(context, indexed.index, calls, cursor, ordered);
            }
            ExpressionNode::Member(member) => {
                visit(context, member.receiver, calls, cursor, ordered)
            }
            ExpressionNode::Mutable(inner) => visit(context, *inner, calls, cursor, ordered),
            ExpressionNode::Range(range) => {
                if range.start.is_valid() {
                    visit(context, range.start, calls, cursor, ordered);
                }
                if range.end.is_valid() {
                    visit(context, range.end, calls, cursor, ordered);
                }
            }
            ExpressionNode::StructLiteral(struct_literal) => {
                for field in expressions.struct_fields(struct_literal.fields) {
                    visit(context, field.value, calls, cursor, ordered);
                }
            }
            ExpressionNode::Unary(unary) => visit(context, unary.operand, calls, cursor, ordered),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }

    let mut ordered = Vec::with_capacity(calls.len());
    let mut cursor = 0;
    visit(context, root, &calls, &mut cursor, &mut ordered);
    // Preserve any unusual unmatched plan records (for example, multiple dyn
    // candidates for one expression occurrence) in their original order.
    ordered.extend(calls.into_iter().skip(cursor));
    ordered
}

fn state_call_is_dispatched(context: &RuntimeDispatchBodyContext, state_call: &StateCall) -> bool {
    // The call's dispatch edge lives on whichever SEGMENT of the source state
    // dispatches it (segment_index varies), so match the source state ignoring
    // segment_index and look for the edge across any of its segments/clones.
    context
        .state_dispatch
        .states
        .iter()
        .filter(|(_, state)| {
            state.key.machine == state_call.source_key.machine
                && state.key.state == state_call.source_key.state
        })
        .any(|(_, state)| {
            context
                .state_dispatch
                .edges
                .span(state.edges)
                .is_some_and(|edges| {
                    edges.iter().any(|edge| {
                        edge.statement_index == state_call.statement_index
                            && matches!(
                                edge.target,
                                RuntimeTransitionTarget::State { key, .. } if key == state_call.target_key
                            )
                    })
                })
        })
}

fn state_call_splits_runtime_body(
    context: &RuntimeDispatchBodyContext,
    state_call: &StateCall,
) -> bool {
    // A dispatched call ends a segment so its callee runs as its own dispatch
    // case(s). This holds for a value-position call (`let n = count(..)`) too: the
    // callee is dispatched (e.g. it loops) and writes its result back to the
    // caller's call-result slot, so the inline expansion must NOT also be emitted.
    matches!(
        state_call.role,
        StateCallRole::Statement | StateCallRole::AssignmentValue
    ) && state_call_is_dispatched(context, state_call)
}

fn append_state_call_body_operation(
    context: &RuntimeDispatchBodyContext,
    state_call: &StateCall,
    operations: &mut Arena<RuntimeDispatchBodyOperation>,
    expressions: &mut ExpressionTable,
    invariant_names: &mut Arena<Identifier>,
    type_references: &mut TypeReferenceTable,
    visiting: &mut BodyVisitingStates,
) {
    if state_call.lowering == StateCallLowering::InlineLeaf {
        operations.insert(body_operation(
            state_call.source_key,
            state_call.statement_index,
            RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
                role: state_call.role,
                call_ordinal: state_call.call_ordinal,
                target_key: state_call.target_key,
                argument_count: state_call.argument_count,
            },
        ));
        append_state_body_operations(
            context,
            state_call.target_key,
            None,
            operations,
            expressions,
            invariant_names,
            type_references,
            visiting,
        );
        append_state_call_result_operation(context, state_call, operations, expressions);
        return;
    }

    if state_has_no_transitions(context, state_call.target_key) {
        operations.insert(body_operation(
            state_call.source_key,
            state_call.statement_index,
            RuntimeDispatchBodyOperationKind::InlineStateCall {
                role: state_call.role,
                call_ordinal: state_call.call_ordinal,
                target_key: state_call.target_key,
                argument_count: state_call.argument_count,
                lowering: state_call.lowering,
            },
        ));
        append_state_body_operations(
            context,
            state_call.target_key,
            None,
            operations,
            expressions,
            invariant_names,
            type_references,
            visiting,
        );
        append_state_call_result_operation(context, state_call, operations, expressions);
        return;
    }

    // The callee has transitions, so its return value is materialised by the branch
    // expansion. Insert the StateCall op FIRST so its arg->param aliases are bound
    // before anything resolves against them.
    operations.insert(body_operation(
        state_call.source_key,
        state_call.statement_index,
        RuntimeDispatchBodyOperationKind::StateCall {
            role: state_call.role,
            call_ordinal: state_call.call_ordinal,
            target_key: state_call.target_key,
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        },
    ));
    // The callee's NON-transition body (local initializers such as
    // `let cells = self.bag.cells.as_mut_slice();`, and mutations through its `&mut`
    // params) is spliced here so it executes before the branch expansion materialises
    // the transition/return -- without it, a machine returning `&mut cells[index]`
    // reads an uninitialized slice descriptor (bad pointer -> segfault). The splice
    // also FLATTENS the callee's nested calls (e.g. a devirtualized tail call) into
    // the caller's body so each becomes its own branching call.
    //
    // This splice is the EXECUTOR for every role except TRANSITION-GUARD calls, whose
    // executor is the branch prelude expansion instead (so the repeated-subject dedupe
    // in omega-runtime-branching can suppress later arms of one transition subject).
    // The two must never both run a callee's statements: that is double execution of
    // its side effects. omega-runtime-branching keeps non-guard preludes EMPTY of
    // callee operations for the same reason.
    if state_call.lowering != StateCallLowering::InlineBranching
        || state_call.role != omega_state_calls::StateCallRole::TransitionGuard
    {
        append_state_body_operations(
            context,
            state_call.target_key,
            None,
            operations,
            expressions,
            invariant_names,
            type_references,
            visiting,
        );
    }
}

fn append_state_call_result_operation(
    context: &RuntimeDispatchBodyContext,
    state_call: &StateCall,
    operations: &mut Arena<RuntimeDispatchBodyOperation>,
    expressions: &mut ExpressionTable,
) {
    if !matches!(
        state_call.role,
        omega_state_calls::StateCallRole::AssignmentValue
            | omega_state_calls::StateCallRole::CallArgument
            | omega_state_calls::StateCallRole::TransitionArgument
            | omega_state_calls::StateCallRole::TransitionGuard
    ) {
        return;
    }

    let value = terminal_state_value_expression(context, state_call.target_key);
    if !value.is_valid() {
        return;
    };
    let value = expressions.copy_from(&context.program.expression_table, value);

    operations.insert(body_operation(
        state_call.source_key,
        state_call.statement_index,
        RuntimeDispatchBodyOperationKind::StateCallResult {
            role: state_call.role,
            call_ordinal: state_call.call_ordinal,
            target_key: state_call.target_key,
            value,
        },
    ));
}

fn terminal_state_value_expression(
    context: &RuntimeDispatchBodyContext,
    target_key: StateKey,
) -> psi_checked_trees::expression::ExpressionHandle {
    let Some(machine) = context
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == target_key.machine)
    else {
        return psi_checked_trees::expression::ExpressionHandle::invalid();
    };
    let Some(state) = context
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == target_key.state)
    else {
        return psi_checked_trees::expression::ExpressionHandle::invalid();
    };
    let statements = context
        .program
        .statement_table
        .statements(state.statement_nodes);
    let Some(statement) = statements.last() else {
        return psi_checked_trees::expression::ExpressionHandle::invalid();
    };
    match statement {
        StatementNode::Expression(expression) => *expression,
        StatementNode::Transition(transition)
            if !transition.continuation.is_valid()
                && matches!(transition.guard, TransitionGuardNode::Always) =>
        {
            match context
                .program
                .statement_table
                .transition_target(transition.target)
            {
                TransitionTargetNode::Value(expression) => *expression,
                _ => psi_checked_trees::expression::ExpressionHandle::invalid(),
            }
        }
        _ => psi_checked_trees::expression::ExpressionHandle::invalid(),
    }
}

fn body_operation(
    source_key: StateKey,
    statement_index: usize,
    kind: RuntimeDispatchBodyOperationKind,
) -> RuntimeDispatchBodyOperation {
    RuntimeDispatchBodyOperation {
        source_key,
        statement_index,
        kind,
    }
}

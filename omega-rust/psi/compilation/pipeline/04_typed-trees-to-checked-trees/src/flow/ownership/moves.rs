use super::type_references::{OperatorResultOwnership, classify_operator_result_ownership};
use crate::flow::FlowOwnershipEventSource;
use validation::canonical_place_from_expression_in_state;
use validation::symbol_type_symbol;

use crate::flow::canonical_place_from_symbol;
use crate::flow::ownership::DirectMoveEventSink;
use crate::flow::ownership::append_move_event_for_place;
use crate::flow::ownership::type_references;
use crate::flow::ownership::type_requires_ownership;
use crate::flow::resolve_operator_for_call;

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use symbols::SymbolHandle;
pub(super) mod observations;

/// Select the consumed value without confusing a loan descriptor with its
/// referent. Context only unwraps a reference when the admitted operand names
/// the exact declared referent type; it cannot supply an unrelated result type.
/// A byte view or `[T]` view is its own descriptor storage, never a pointer to
/// a referent the operand could consume: storing one into a view-typed place
/// (`slot.out = text`) copies the descriptor even though the assignment's
/// expected type is the place's referent (`assignment_value_type`).
fn consumed_value_type(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement: usize,
    expression: ExpressionHandle,
    expected: typed_trees::types::TypeReferenceHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let actual =
        validation::expression_type_reference_in_state(program, state, statement, expression)?;
    if expected.is_valid()
        && validation::stores_reference_shape(program, actual, &[])
        && let Some(shell) = validation::reference_shell(program, actual)
        && program.normalized_type_identity(shell.referee)
            == program.normalized_type_identity(expected)
    {
        return Some(expected);
    }
    Some(actual)
}

pub(super) fn append_move_events_for_expression(
    program: &dyn ::validation::CheckingInput,
    sink: &mut DirectMoveEventSink<'_>,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    expected_type: typed_trees::types::TypeReferenceHandle,
    source: FlowOwnershipEventSource,
) {
    // A case-membership test observes a saved place; it does not extract the
    // nominal value whose tag is read. Restrict this to static symbol paths so
    // calls and computed indexes still expose their owned argument transfers.
    if let Some((subject, _)) = crate::proof::exact_outcome_case_test(program, expression)
        && observations::selected_operator(sink, state_symbol, statement_index, expression)
            .is_none()
        && canonical_place_from_expression_in_state(program, state_symbol, statement_index, subject)
            .is_some_and(|place| {
                matches!(place.root, facts::PlaceRoot::Symbol(_))
                    && place.segments.iter().all(|segment| {
                        matches!(
                            segment,
                            facts::PlaceSegment::Field { .. }
                                | facts::PlaceSegment::Case { .. }
                                | facts::PlaceSegment::FixedIndex { .. }
                        )
                    })
            })
    {
        return;
    }

    if type_references::intrinsic_enum_equality(program, state_symbol, expression)
        || selected_meaning_borrows_operands(program, expression)
    {
        let Some(binary) = program.expression_table.binary_node(expression) else {
            unreachable!()
        };
        for operand in [binary.left, binary.right] {
            observations::append(
                program,
                sink,
                state_symbol,
                statement_index,
                operand,
                source,
            );
        }
        return;
    }

    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Indexed(_)
    ) && observations::append_indexed_operands(
        program,
        sink,
        state_symbol,
        statement_index,
        expression,
        source,
    ) == Some(true)
    {
        return;
    }

    let value_type = consumed_value_type(
        program,
        state_symbol,
        statement_index,
        expression,
        expected_type,
    );
    if type_references::expression_is_place_like(program, expression)
        && value_type
            .map(|reference| type_requires_ownership(program, reference))
            .unwrap_or(true)
    {
        if let Some(place) = canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            expression,
        ) {
            // A value read through borrowed storage can be a detached copy
            // rather than a custody transfer: an indexed element read across a
            // purely shared chain observes its element, and a `Binding<R>`/
            // boundary-trait receiver call marshals a caller-owned ABI copy of
            // the argument's runtime contents. Every other reach keeps the
            // real move so the borrowed-window and shared-loan rejections
            // stand.
            if observations::detached_borrowed_copy_admitted(
                program,
                state_symbol,
                statement_index,
                &place,
                source,
            ) {
                observations::append(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    expression,
                    source,
                );
            } else {
                append_move_event_for_place(
                    sink,
                    place,
                    source,
                    expression,
                    value_type.unwrap_or_default(),
                );
            }
        }
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            // A discriminant dispatch reads only the subject's case tag, like
            // the membership-test exemption above: the observed place is not
            // extracted. Subjects are parameter field paths, immutable locals,
            // or fresh constructions, none of which move owned operands out of
            // the tag read.
            let tag_observed =
                validation::match_case_dispatch(program, sink.machine, sink.state, dispatch)
                    .is_some();
            if !tag_observed {
                append_move_events_for_expression(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    dispatch.subject,
                    Default::default(),
                    source,
                );
            }
            for (ordinal, arm) in program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .enumerate()
            {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    append_move_events_for_expression(
                        program,
                        sink,
                        state_symbol,
                        statement_index,
                        pattern,
                        Default::default(),
                        source,
                    );
                }
                let enclosing_arm = sink.source_arm;
                sink.source_arm = dispatch.arms.handle_at(ordinal as u32);
                append_move_events_for_expression(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    arm.value,
                    expected_type,
                    source,
                );
                sink.source_arm = enclosing_arm;
            }
        }
        ExpressionNode::Atomic(atomic) => append_move_events_for_expression(
            program,
            sink,
            state_symbol,
            statement_index,
            atomic.value,
            expected_type,
            source,
        ),
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                append_move_events_for_expression(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    *value,
                    if expected_type.is_valid() {
                        match program.type_reference_table.type_reference(expected_type) {
                            typed_trees::types::TypeReferenceNode::FixedArray {
                                element_type,
                                ..
                            } => *element_type,
                            _ => Default::default(),
                        }
                    } else {
                        Default::default()
                    },
                    source,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            append_move_events_for_expression(
                program,
                sink,
                state_symbol,
                statement_index,
                binary.left,
                Default::default(),
                source,
            );
            append_move_events_for_expression(
                program,
                sink,
                state_symbol,
                statement_index,
                binary.right,
                Default::default(),
                source,
            );
        }
        ExpressionNode::Cast(cast) => append_move_events_for_expression(
            program,
            sink,
            state_symbol,
            statement_index,
            cast.value,
            Default::default(),
            source,
        ),
        ExpressionNode::Unary(unary) => append_move_events_for_expression(
            program,
            sink,
            state_symbol,
            statement_index,
            unary.operand,
            Default::default(),
            source,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                append_move_events_for_expression(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    field.value,
                    validation::selected_construction_field_type(
                        program,
                        struct_literal,
                        field.field_symbol,
                    )
                    .unwrap_or_default(),
                    source,
                );
            }
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                append_move_events_for_expression(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    range.start,
                    Default::default(),
                    source,
                );
            }
            if range.end.is_valid() {
                append_move_events_for_expression(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    range.end,
                    Default::default(),
                    source,
                );
            }
        }
        // Nested calls retain their parameter-aligned owned arguments.
        // Ordinary state/signature calls are already owned by call-flow;
        // the helper avoids recording those transfers twice.
        ExpressionNode::Call(call) => {
            append_move_events_for_call_arguments(
                program,
                sink,
                state_symbol,
                statement_index,
                call,
                source,
            );
        }
        ExpressionNode::Borrow(borrow) => observations::append(
            program,
            sink,
            state_symbol,
            statement_index,
            borrow.target,
            source,
        ),
        ExpressionNode::Name(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Indexed(_)
        | typed_trees::spelled_literal_expression_pattern!() => {}
    }
}

/// Evaluate nested call operands using their declared ownership policy.
/// Static type/module receivers name declarations rather than runtime storage.
/// A binary expression whose authored selection resolved to a machine that
/// takes every operand by shared reference observes its operands, exactly as
/// a call to that machine lends its arguments: `a == b` over an `Equatable`
/// implementation with an authored `equals` row selects that row, and neither
/// side moves.
fn selected_meaning_borrows_operands(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget as Target;
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Binary(_)
    ) {
        return false;
    }
    let mut targets = program
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| program.authored_declaration_selections().get(occurrence))
        .map(|selection| match selection.target() {
            Target::Resolved(target) => Some(target.selected_symbol()),
            _ => None,
        });
    let (Some(Some(target)), None) = (targets.next(), targets.next()) else {
        return false;
    };
    validation::call_target_parameters(program, target).is_some_and(|parameters| {
        parameters.len() == 2
            && parameters.iter().all(|parameter| {
                matches!(
                    program
                        .type_reference_table
                        .type_reference(parameter.type_reference),
                    typed_trees::types::TypeReferenceNode::Reference {
                        access: language_semantics::ReferenceAccess::Shared,
                        ..
                    }
                )
            })
    })
}

fn append_move_events_for_call_arguments(
    program: &dyn ::validation::CheckingInput,
    sink: &mut DirectMoveEventSink<'_>,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call: &typed_trees::expression::TableCallExpression,
    source: FlowOwnershipEventSource,
) {
    // Calls with an ordinary state or a bodyless callable signature are owned
    // by the call-flow argument-move pass. Expression recursion handles only
    // operators/unresolved targets; otherwise a boundary-trait value call
    // would record every owned argument twice.
    if validation::call_target_parameters(program, call.target_symbol).is_some()
        || expression_call_is_owned_by_call_flow(program, state_symbol, call)
    {
        return;
    }

    let receiver_is_static_path = receiver_expression_is_static_type_path(program, call.receiver);
    let arguments = program.expression_table.expression_handles(call.arguments);
    let operator = resolve_operator_for_call(
        program,
        call.target_symbol,
        receiver_expression_static_path_segments(program, call.receiver).as_deref(),
        call.target.as_str(),
        arguments.len(),
        call.receiver.is_valid() && !receiver_is_static_path,
    );
    let policy = operator_call_ownership_policy(
        program,
        operator,
        arguments.len(),
        call.receiver.is_valid() && !receiver_is_static_path,
    );

    if call.receiver.is_valid() && !receiver_is_static_path && policy.receiver_transfers() {
        append_move_events_for_expression(
            program,
            sink,
            state_symbol,
            statement_index,
            call.receiver,
            policy.receiver().value_type,
            source,
        );
    }

    for (ordinal, argument) in arguments.iter().enumerate() {
        // When the callee's parameter type is known and copies/borrows (does
        // not own), the argument transfers nothing.
        if !policy.positional_transfers(ordinal) {
            continue;
        }

        append_move_events_for_expression(
            program,
            sink,
            state_symbol,
            statement_index,
            *argument,
            policy.positional(ordinal).value_type,
            source,
        );
    }
}

/// Whether borrow-call discovery owns this expression call even though the
/// table expression itself does not retain the final target-state symbol.
///
/// Attached method calls may carry an invalid `target_symbol` in the typed
/// expression and resolve through the receiver's exact type instead. Borrow
/// discovery performs that resolution and emits the parameter-aligned moves.
/// Expression recursion must make the same classification or its unknown-call
/// fallback will incorrectly transfer a borrowed receiver wholesale.
fn expression_call_is_owned_by_call_flow(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    let Some(state) = program.state_by_symbol(state_symbol) else {
        return false;
    };
    let Some(machine) = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == state.symbol)
    }) else {
        return false;
    };
    let (receiver_symbol, receiver_path) = validation::call_receiver_parts(program, call.receiver);
    // A named boundary operator is an ordinary requirement call: borrow
    // discovery keys it by its declaration symbol and the call-flow pass moves
    // its owned arguments. Classify it the same way here.
    let boundary_operator = typed_trees::operator::resolve_named_expression_call(program, call)
        .filter(|operator| operator.is_boundary);
    validation::resolve_state_call_target(
        program,
        machine,
        state,
        receiver_symbol,
        boundary_operator.map_or(call.target_symbol, |operator| operator.symbol),
        receiver_path.as_deref(),
        &call.target,
    )
    .is_valid()
        || validation::receiver_can_dispatch_to_machine(
            program,
            machine,
            state,
            receiver_symbol,
            receiver_path.as_deref(),
        )
}

/// Append type-aware move events for the owned by-value arguments of a
/// statement-level call that resolves to an operator/boundary definition
/// (`Codec::encode(payload);`, `text.push_str(suffix);`).
///
/// Statement calls that dispatch to a machine state are owned by the call-flow
/// pass -- their discovered `BorrowCallFact`s drive
/// `append_call_ownership_events` -- so this only handles operator/boundary
/// targets, the one statement-call class with no borrow-call fact. The
/// callee's declared parameter types decide per argument, exactly as in
/// [`append_move_events_for_call_arguments`]: a copy parameter reads its
/// argument by value and transfers nothing. An owned by-value `self` parameter
/// consumes the receiver, so it gets a receiver-place move; the common
/// borrowed receiver (`&self`/`&mut self`) transfers nothing.
pub(in crate::flow::ownership) fn append_move_events_for_operator_statement_call(
    program: &dyn ::validation::CheckingInput,
    sink: &mut DirectMoveEventSink<'_>,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call: &typed_trees::statement::TableCall,
    source: FlowOwnershipEventSource,
) {
    // A valid receiver symbol that resolves to a typed value (a local,
    // parameter, or field) is a method-form receiver place; a static path
    // receiver (`Sink::consume(...)`) names no runtime value.
    // Statement-call argument spans live in the statement table (unlike
    // expression-call spans, which live in the expression table).
    let arguments = program.statement_table.expression_handles(call.arguments);
    let Some(resolved) = crate::flow::resolve_operator_statement_call(program, call) else {
        // Not a known operator: a machine/state statement call (owned by the
        // call-flow pass) or an unresolved target (no policy to apply).
        return;
    };
    let policy = operator_call_ownership_policy(
        program,
        Some(resolved.operator),
        arguments.len(),
        resolved.receiver_is_value,
    );

    if resolved.receiver_is_value
        && policy.receiver_transfers()
        && let Some(place) = canonical_place_from_symbol(call.receiver_symbol)
    {
        append_move_event_for_place(
            sink,
            place,
            source,
            Default::default(),
            policy.receiver().value_type,
        );
    }

    for (ordinal, argument) in arguments.iter().enumerate() {
        if !policy.positional_transfers(ordinal) {
            continue;
        }

        append_move_events_for_expression(
            program,
            sink,
            state_symbol,
            statement_index,
            *argument,
            policy.positional(ordinal).value_type,
            source,
        );
    }
}

/// The receiver/argument ownership policy of one call site, derived from the
/// resolved operator definition's declared parameter types. `Unknown` (no
/// resolved operator) preserves the legacy behavior: every value operand
/// decides by its own type through the recursive descent.
#[derive(Clone, Copy)]
struct OperatorOperandOwnership {
    value_type: typed_trees::types::TypeReferenceHandle,
    transfers: bool,
}

impl OperatorOperandOwnership {
    fn unknown() -> Self {
        Self {
            value_type: Default::default(),
            transfers: true,
        }
    }
}

enum OperatorCallOwnershipPolicy {
    Unknown,
    Known {
        receiver: OperatorOperandOwnership,
        positional: Vec<OperatorOperandOwnership>,
    },
}

impl OperatorCallOwnershipPolicy {
    fn receiver(&self) -> OperatorOperandOwnership {
        match self {
            Self::Unknown => OperatorOperandOwnership::unknown(),
            Self::Known { receiver, .. } => *receiver,
        }
    }

    fn positional(&self, ordinal: usize) -> OperatorOperandOwnership {
        match self {
            Self::Unknown => OperatorOperandOwnership::unknown(),
            Self::Known { positional, .. } => positional
                .get(ordinal)
                .copied()
                .unwrap_or_else(OperatorOperandOwnership::unknown),
        }
    }

    fn receiver_transfers(&self) -> bool {
        self.receiver().transfers
    }
    fn positional_transfers(&self, ordinal: usize) -> bool {
        self.positional(ordinal).transfers
    }
}

/// Line an operator's declared parameters up against a call site's receiver
/// and positional arguments.
///
/// Three shapes exist: a declared `self` parameter binds the receiver directly;
/// a method-form call on a value receiver (`text.push_str(suffix)`) binds the
/// declaration's leading parameter (`text: &mut String`) to the receiver and
/// the rest positionally; a static-path call binds every parameter
/// positionally. Each binding transfers only when its declared type owns.
fn operator_call_ownership_policy(
    program: &dyn ::validation::CheckingInput,
    operator: Option<&typed_trees::operator::OperatorDefinition>,
    argument_count: usize,
    has_value_receiver: bool,
) -> OperatorCallOwnershipPolicy {
    let Some(operator) = operator else {
        return OperatorCallOwnershipPolicy::Unknown;
    };

    let parameters = program.operator_parameters(operator);
    let positional: Vec<OperatorOperandOwnership> = parameters
        .iter()
        .map(|parameter| OperatorOperandOwnership {
            value_type: parameter.type_reference,
            transfers: type_requires_ownership(program, parameter.type_reference),
        })
        .collect();

    if has_value_receiver && positional.len() == argument_count + 1 {
        return OperatorCallOwnershipPolicy::Known {
            receiver: positional[0],
            positional: positional[1..].to_vec(),
        };
    }

    OperatorCallOwnershipPolicy::Known {
        receiver: OperatorOperandOwnership {
            value_type: Default::default(),
            transfers: false,
        },
        positional,
    }
}

/// Whether a call's receiver expression is a static type/module path
/// (`String::with_capacity(...)`) rather than a runtime value. A static path
/// names no place, so it can never be moved from.
fn receiver_expression_is_static_type_path(
    program: &dyn ::validation::CheckingInput,
    receiver: ExpressionHandle,
) -> bool {
    receiver_expression_static_path_segments(program, receiver).is_some()
}

/// The textual segments of a static type/module path receiver, or `None` when
/// the receiver is absent or is a runtime value expression. A bare `Name` path
/// counts as static exactly when its resolved symbol has no value type (it is
/// not a local, parameter, field, or contained object).
fn receiver_expression_static_path_segments(
    program: &dyn ::validation::CheckingInput,
    receiver: ExpressionHandle,
) -> Option<Vec<&str>> {
    if !receiver.is_valid() {
        return None;
    }

    match program.expression_table.expression(receiver) {
        ExpressionNode::Borrow(inner) => {
            receiver_expression_static_path_segments(program, inner.target)
        }
        ExpressionNode::Name(path) => {
            let value_symbol =
                validation::first_valid_name_path_symbol(path, &program.expression_table)
                    .filter(|symbol| symbol_type_symbol(program, *symbol).is_some());
            if value_symbol.is_some() {
                return None;
            }
            Some(
                program
                    .expression_table
                    .name_path_members(path.members)
                    .iter()
                    .map(|identifier| identifier.as_str())
                    .collect(),
            )
        }
        _ => None,
    }
}

/// Whether a `let`-initializer expression produces a freshly owned value through
/// a boundary/slice/string/collection operator call (as opposed to borrowing a
/// view or reading an existing place).
///
/// Used to decide whether an owned binding's initializer warrants a move event
/// into the bound local. Operator results are classified through
/// [`classify_operator_result_ownership`], keeping operator-result ownership
/// policy in one place; a place-like initializer is excluded because its
/// transfer is already recorded by the source-side move.
pub(in crate::flow::ownership) fn initializer_produces_owned_value(
    program: &dyn ::validation::CheckingInput,
    initializer: ExpressionHandle,
) -> bool {
    if !initializer.is_valid() {
        return false;
    }

    let call = match program.expression_table.expression(initializer) {
        ExpressionNode::Call(call) => call,
        _ => return false,
    };

    let static_segments = receiver_expression_static_path_segments(program, call.receiver);
    let arguments = program.expression_table.expression_handles(call.arguments);
    resolve_operator_for_call(
        program,
        call.target_symbol,
        static_segments.as_deref(),
        call.target.as_str(),
        arguments.len(),
        call.receiver.is_valid() && static_segments.is_none(),
    )
    .map(|operator| {
        classify_operator_result_ownership(program, operator.return_type)
            == OperatorResultOwnership::OwnedValue
    })
    .unwrap_or(false)
}

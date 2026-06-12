use super::type_references::{OperatorResultOwnership, classify_operator_result_ownership};
use super::*;

pub(super) fn append_move_events_for_expression(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    source: FlowOwnershipEventSource,
) {
    if type_references::expression_requires_ownership(
        program,
        state_symbol,
        statement_index,
        expression,
    ) {
        if let Some(place) = canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            expression,
        ) {
            append_move_event_for_place(program, ctx, place, source);
        }
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                append_move_events_for_expression(
                    program,
                    ctx,
                    state_symbol,
                    statement_index,
                    *value,
                    source,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            append_move_events_for_expression(
                program,
                ctx,
                state_symbol,
                statement_index,
                binary.left,
                source,
            );
            append_move_events_for_expression(
                program,
                ctx,
                state_symbol,
                statement_index,
                binary.right,
                source,
            );
        }
        ExpressionNode::Cast(cast) => append_move_events_for_expression(
            program,
            ctx,
            state_symbol,
            statement_index,
            cast.value,
            source,
        ),
        ExpressionNode::Unary(unary) => append_move_events_for_expression(
            program,
            ctx,
            state_symbol,
            statement_index,
            unary.operand,
            source,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                append_move_events_for_expression(
                    program,
                    ctx,
                    state_symbol,
                    statement_index,
                    field.value,
                    source,
                );
            }
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                append_move_events_for_expression(
                    program,
                    ctx,
                    state_symbol,
                    statement_index,
                    range.start,
                    source,
                );
            }
            if range.end.is_valid() {
                append_move_events_for_expression(
                    program,
                    ctx,
                    state_symbol,
                    statement_index,
                    range.end,
                    source,
                );
            }
        }
        // A call appearing in a *value* sub-expression position (a nested
        // operator/boundary or state call used as an aggregate element/field, a
        // binary or range operand, or a cast operand) still transfers ownership
        // of any owned by-value arguments it consumes. Recursion reaches such a
        // call only *through* an enclosing value expression; the call-flow
        // discovery pass only records argument moves for state borrow calls, so
        // for non-state (operator/boundary) calls reached this way the owned
        // argument transfers would otherwise leave no ownership event. Descend
        // into the call's owned by-value arguments here.
        //
        // State borrow calls (`find_state(target) == Some`) are intentionally
        // excluded: their by-value argument transfers are emitted by
        // `append_call_ownership_events` from the discovered `BorrowCallFact`s,
        // so descending into them here would double-count.
        ExpressionNode::Call(call) => {
            append_move_events_for_call_arguments(
                program,
                ctx,
                state_symbol,
                statement_index,
                call,
                source,
            );
        }
        ExpressionNode::Mutable(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}

/// Append type-aware move events for the owned by-value arguments (and receiver)
/// of a call that is itself reached as a value sub-expression.
///
/// State borrow calls are skipped because the call-flow pass already records
/// their argument transfers from the discovered borrow-call facts. For an
/// operator/boundary call the per-parameter ownership policy comes from the
/// callee's declared parameter types (the same source of truth as
/// [`classify_operator_result_ownership`]); a parameter whose type copies or
/// borrows reads its argument without transferring ownership, so it produces no
/// move event. When no operator declaration is found the argument expression's
/// own type-aware policy decides via the recursive descent.
///
/// A static type-name receiver (`String::with_capacity(8)`) names a type, not a
/// runtime value: it is never a place and never moves, so it is excluded from
/// the receiver descent regardless of the callee.
fn append_move_events_for_call_arguments(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call: &omega_typed_trees::expression::TableCallExpression,
    source: FlowOwnershipEventSource,
) {
    // State borrow calls are owned by the call-flow argument-move pass.
    if find_state(program, call.target_symbol).is_some() {
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
            ctx,
            state_symbol,
            statement_index,
            call.receiver,
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
            ctx,
            state_symbol,
            statement_index,
            *argument,
            source,
        );
    }
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
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call: &omega_typed_trees::statement::TableCall,
    source: FlowOwnershipEventSource,
) {
    // A valid receiver symbol that resolves to a typed value (a local,
    // parameter, or field) is a method-form receiver place; a static path
    // receiver (`Sink::consume(...)`) names no runtime value.
    let receiver_is_value = call.receiver_symbol.is_valid()
        && symbol_type_symbol(program, call.receiver_symbol).is_some();
    let receiver_segments: Vec<&str> = if receiver_is_value {
        Vec::new()
    } else {
        program
            .statement_table
            .name_path_members(call.receiver)
            .iter()
            .map(|identifier| identifier.as_str())
            .collect()
    };

    // Statement-call argument spans live in the statement table (unlike
    // expression-call spans, which live in the expression table).
    let arguments = program.statement_table.expression_handles(call.arguments);
    let Some(operator) = resolve_operator_for_call(
        program,
        call.target_symbol,
        (!receiver_segments.is_empty()).then_some(receiver_segments.as_slice()),
        call.target.as_str(),
        arguments.len(),
        receiver_is_value,
    ) else {
        // Not a known operator: a machine/state statement call (owned by the
        // call-flow pass) or an unresolved target (no policy to apply).
        return;
    };
    let policy =
        operator_call_ownership_policy(program, Some(operator), arguments.len(), receiver_is_value);

    if receiver_is_value && policy.receiver_transfers() {
        if let Some(place) = canonical_place_from_symbol(call.receiver_symbol) {
            append_move_event_for_place(program, ctx, place, source);
        }
    }

    for (ordinal, argument) in arguments.iter().enumerate() {
        if !policy.positional_transfers(ordinal) {
            continue;
        }

        append_move_events_for_expression(
            program,
            ctx,
            state_symbol,
            statement_index,
            *argument,
            source,
        );
    }
}

/// The receiver/argument ownership policy of one call site, derived from the
/// resolved operator definition's declared parameter types. `Unknown` (no
/// resolved operator) preserves the legacy behavior: every value operand
/// decides by its own type through the recursive descent.
enum OperatorCallOwnershipPolicy {
    Unknown,
    Known {
        receiver_transfers: bool,
        positional_transfers: Vec<bool>,
    },
}

impl OperatorCallOwnershipPolicy {
    fn receiver_transfers(&self) -> bool {
        match self {
            Self::Unknown => true,
            Self::Known {
                receiver_transfers, ..
            } => *receiver_transfers,
        }
    }

    fn positional_transfers(&self, ordinal: usize) -> bool {
        match self {
            Self::Unknown => true,
            Self::Known {
                positional_transfers,
                ..
            } => positional_transfers.get(ordinal).copied().unwrap_or(true),
        }
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
    program: &omega_typed_trees::TypedTrees,
    operator: Option<&omega_typed_trees::operator::OperatorDefinition>,
    argument_count: usize,
    has_value_receiver: bool,
) -> OperatorCallOwnershipPolicy {
    let Some(operator) = operator else {
        return OperatorCallOwnershipPolicy::Unknown;
    };

    let parameters = program.operator_parameters(operator);
    let self_transfer = parameters
        .iter()
        .find(|parameter| parameter.is_self)
        .map(|parameter| type_requires_ownership(program, parameter.type_reference));
    let positional: Vec<bool> = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .map(|parameter| type_requires_ownership(program, parameter.type_reference))
        .collect();

    if let Some(receiver_transfers) = self_transfer {
        return OperatorCallOwnershipPolicy::Known {
            receiver_transfers,
            positional_transfers: positional,
        };
    }

    if has_value_receiver && positional.len() == argument_count + 1 {
        return OperatorCallOwnershipPolicy::Known {
            receiver_transfers: positional[0],
            positional_transfers: positional[1..].to_vec(),
        };
    }

    OperatorCallOwnershipPolicy::Known {
        receiver_transfers: false,
        positional_transfers: positional,
    }
}

/// Whether a call's receiver expression is a static type/module path
/// (`String::with_capacity(...)`) rather than a runtime value. A static path
/// names no place, so it can never be moved from.
fn receiver_expression_is_static_type_path(
    program: &omega_typed_trees::TypedTrees,
    receiver: ExpressionHandle,
) -> bool {
    receiver_expression_static_path_segments(program, receiver).is_some()
}

/// The textual segments of a static type/module path receiver, or `None` when
/// the receiver is absent or is a runtime value expression. A bare `Name` path
/// counts as static exactly when its resolved symbol has no value type (it is
/// not a local, parameter, field, or contained object).
fn receiver_expression_static_path_segments<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    receiver: ExpressionHandle,
) -> Option<Vec<&'program str>> {
    if !receiver.is_valid() {
        return None;
    }

    match program.expression_table.expression(receiver) {
        ExpressionNode::Mutable(inner) => receiver_expression_static_path_segments(program, *inner),
        ExpressionNode::Name(path) => {
            let value_symbol =
                crate::lookup::first_valid_name_path_symbol(path, &program.expression_table)
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
    program: &omega_typed_trees::TypedTrees,
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

/// Resolve the operator/boundary definition a call dispatches to.
///
/// A direct symbol match wins when typing resolved one. Source-level operator
/// calls usually carry NO resolved operator symbol today (call-target
/// resolution binds machine states and builtins, not operators), so this falls
/// back to matching the call's spelled path -- the static receiver segments
/// plus the target name -- against each operator's declared name path, and
/// requires the argument count to fit the declaration's shape. The match must
/// be unambiguous: competing overloads yield `None` (no event) rather than a
/// guessed policy.
fn resolve_operator_for_call<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
    static_receiver_segments: Option<&[&str]>,
    target_name: &str,
    argument_count: usize,
    has_value_receiver: bool,
) -> Option<&'program omega_typed_trees::operator::OperatorDefinition> {
    if target_symbol.is_valid() {
        if let Some(operator) = program
            .operators()
            .iter()
            .find(|operator| operator.symbol == target_symbol)
        {
            return Some(operator);
        }
    }

    let mut candidates = program.operators().iter().filter(|operator| {
        operator_path_matches_call(program, operator, static_receiver_segments, target_name)
            && operator_arity_fits_call(program, operator, argument_count, has_value_receiver)
    });

    let first = candidates.next()?;
    candidates.next().is_none().then_some(first)
}

/// Whether an operator's declared name path spells this call: the last segment
/// is the call target, and -- for a static-path call -- the leading segments
/// equal the receiver path. A value-receiver method call constrains only the
/// final segment; arity matching narrows the rest.
fn operator_path_matches_call(
    program: &omega_typed_trees::TypedTrees,
    operator: &omega_typed_trees::operator::OperatorDefinition,
    static_receiver_segments: Option<&[&str]>,
    target_name: &str,
) -> bool {
    let path = program.operator_path_members(operator.name);
    let Some((last, prefix)) = path.split_last() else {
        return false;
    };
    if last.as_str() != target_name {
        return false;
    }

    match static_receiver_segments {
        Some(segments) => {
            prefix.len() == segments.len()
                && prefix
                    .iter()
                    .zip(segments.iter())
                    .all(|(member, segment)| member.as_str() == *segment)
        }
        None => true,
    }
}

/// Whether the call site's operand count fits the operator declaration: a
/// declared `self` parameter or a leading method-form parameter absorbs the
/// value receiver, otherwise every parameter binds positionally.
fn operator_arity_fits_call(
    program: &omega_typed_trees::TypedTrees,
    operator: &omega_typed_trees::operator::OperatorDefinition,
    argument_count: usize,
    has_value_receiver: bool,
) -> bool {
    let parameters = program.operator_parameters(operator);
    let has_self = parameters.iter().any(|parameter| parameter.is_self);
    let positional = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();

    if has_self {
        return has_value_receiver && positional == argument_count;
    }
    if has_value_receiver {
        return positional == argument_count + 1;
    }
    positional == argument_count
}

use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_checked_trees::name::Identifier;
use omega_control_flow::{Operation, StateKey, StateParameterFlow};
use omega_core::symbols::SymbolHandle;
use omega_platform_interface::HostCall;
use omega_state_calls::StateCall;
use omega_state_storage::StateMutation;

fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}

pub(super) fn host_call_for_statement<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan HostCall> {
    input
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            state_key_matches_statement_source(host_call.source_key, source_key)
                && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call)
}

pub(super) fn state_call_for_statement<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .statement_call(source_key, statement_index)
}

pub(super) fn state_assignment_value_call<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .assignment_value_call(source_key, statement_index)
}

pub(super) fn state_assignment_value_call_by_ordinal<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .assignment_value_call_by_ordinal(source_key, statement_index, call_ordinal)
}

pub(super) fn state_call_argument_call_by_ordinal<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .call_argument_call_by_ordinal(source_key, statement_index, call_ordinal)
}

pub(super) fn state_transition_guard_call<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .transition_guard_call(source_key, statement_index)
}

pub(super) fn state_transition_argument_call<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .transition_argument_call(source_key, statement_index)
}

pub(super) fn state_transition_argument_call_by_ordinal<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .transition_argument_call_by_ordinal(source_key, statement_index, call_ordinal)
}

pub(super) fn state_parameters<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    state_key: StateKey,
) -> &'plan [StateParameterFlow] {
    input
        .control_flow
        .state_by_key(state_key)
        .map(|state| input.control_flow.state_parameters(state))
        .unwrap_or(&[])
}

pub(super) fn state_operations<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    state_key: StateKey,
) -> Option<&'plan [Operation]> {
    input
        .control_flow
        .state_by_key(state_key)
        .and_then(|state| input.control_flow.operations.span(state.operations))
}

/// The machine a method call's receiver statically dispatches to, when the
/// receiver's concrete data type is derivable from the receiver EXPRESSION alone:
/// a reference parameter (`s.code()` -- the param symbol's declared type) or a
/// field of an attached data record (`self.c.code()`, including an alias- or
/// binding-resolved `(mut self.c).code()` -- the contained object's type).
/// `None` when the type cannot be derived (e.g. a still-`dyn Trait` receiver), so
/// callers keep their unfiltered candidate set.
///
/// This is the discriminator that keeps SAME-NAMED methods on DIFFERENT data
/// types apart: leaf-expansion candidates are matched by target state NAME
/// (`code`), which both `Circle::code` and `Square::code` answer to -- without
/// the receiver's type the lexically-first impl would win at every call site.
pub(super) fn static_receiver_machine_for_call(
    input: &InstructionSelectionInput<'_>,
    receiver: Option<&Expression>,
    target_symbol: SymbolHandle,
    target_name: &str,
) -> Option<SymbolHandle> {
    let type_name = static_receiver_type_name(input, receiver?)?;
    attached_machine_with_target_state(input, &type_name, target_symbol, target_name)
}

/// Table-expression twin of [`static_receiver_machine_for_call`].
pub(super) fn static_receiver_machine_for_table_call(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    receiver: ExpressionHandle,
    target_symbol: SymbolHandle,
    target_name: &str,
) -> Option<SymbolHandle> {
    if !receiver.is_valid() {
        return None;
    }
    let type_name = static_table_receiver_type_name(input, expressions, receiver)?;
    attached_machine_with_target_state(input, &type_name, target_symbol, target_name)
}

/// The static data type NAME a receiver expression resolves to, keyed on the
/// SYMBOL it names (program-unique, so no source-state context is needed): a
/// state parameter's declared type, or a contained object's type (an
/// attached-data field whose type has an attached machine).
fn static_receiver_type_name(
    input: &InstructionSelectionInput<'_>,
    receiver: &Expression,
) -> Option<Identifier> {
    match receiver {
        Expression::Mutable(inner) => static_receiver_type_name(input, inner),
        Expression::Member(member) => receiver_symbol_type_name(input, member.member_symbol),
        Expression::Name(path) => {
            let symbol = if path.symbol().is_valid() {
                path.symbol()
            } else {
                path.head_symbol()
            };
            receiver_symbol_type_name(input, symbol)
        }
        _ => None,
    }
}

fn static_table_receiver_type_name(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    receiver: ExpressionHandle,
) -> Option<Identifier> {
    match expressions.expression(receiver) {
        ExpressionNode::Mutable(inner) => static_table_receiver_type_name(input, expressions, *inner),
        ExpressionNode::Member(member) => {
            receiver_symbol_type_name(input, member.member_symbol)
        }
        ExpressionNode::Name(path) => {
            let symbol = if path.symbol.is_valid() {
                path.symbol
            } else {
                path.head_symbol
            };
            receiver_symbol_type_name(input, symbol)
        }
        _ => None,
    }
}

fn receiver_symbol_type_name(
    input: &InstructionSelectionInput<'_>,
    symbol: SymbolHandle,
) -> Option<Identifier> {
    if !symbol.is_valid() {
        return None;
    }
    let parameter_type = input.control_flow.states.iter().find_map(|(_, state)| {
        input
            .control_flow
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == symbol)
            .map(|parameter| parameter.type_name.clone())
    });
    parameter_type.or_else(|| {
        input.control_flow.machines.iter().find_map(|(_, machine)| {
            input
                .control_flow
                .machine_contains(machine)
                .iter()
                .find(|contained| contained.symbol == symbol)
                .map(|contained| contained.type_name.clone())
        })
    })
}

/// The machine ATTACHED to data type `type_name` that implements the call's
/// target method (matched by state symbol or name), or `None`.
fn attached_machine_with_target_state(
    input: &InstructionSelectionInput<'_>,
    type_name: &Identifier,
    target_symbol: SymbolHandle,
    target_name: &str,
) -> Option<SymbolHandle> {
    input.control_flow.machines.iter().find_map(|(_, machine)| {
        if machine.attached_data.as_ref() != Some(type_name) {
            return None;
        }
        input
            .control_flow
            .states
            .span(machine.states)?
            .iter()
            .find(|state| {
                (target_symbol.is_valid() && state.key.state == target_symbol)
                    || state.name.as_str() == target_name
            })
            .map(|_| machine.symbol)
    })
}

pub(super) fn state_mutation_for_statement<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateMutation> {
    input
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            state_key_matches_statement_source(mutation.source_key, source_key)
                && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}

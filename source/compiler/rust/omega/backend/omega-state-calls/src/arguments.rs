use crate::StateCallPlanningContext;
use omega_control_flow::StateKey;
use omega_control_flow::StateParameterFlow;
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;

use super::lookups::state_flow_from_key;
use super::{StateCallArgument, StateCallArgumentKind, StateCallDynamicConformance, StateCallRole};

pub(crate) fn build_call_arguments(
    context: &StateCallPlanningContext,
    output_expressions: &mut ExpressionTable,
    output_arguments: &mut Arena<StateCallArgument>,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
    target_key: StateKey,
    required: bool,
    raw_arguments: HandleSpan<ExpressionHandle>,
) -> HandleSpan<StateCallArgument> {
    let parameters = state_parameters(context, target_key);
    let raw_arguments = context
        .control_flow
        .expressions
        .expression_handles(raw_arguments);
    // State-graph parameters omit an attached machine's implicit `self`.
    // Static attached calls supply that receiver as one surplus leading
    // argument (`Receipt::ack(receipt)`); instance calls obtain it from their
    // receiver expression and have exactly the non-self parameter count.
    let has_explicit_attached_self = raw_arguments.len() == parameters.len().saturating_add(1)
        && context.control_flow.machines.iter().any(|(_, machine)| {
            machine.symbol == target_key.machine && machine.attached_data.is_some()
        });

    let mut arguments = HandleSpan::empty();

    for (index, expression) in raw_arguments.iter().enumerate() {
        let parameter_index = index.saturating_sub(usize::from(has_explicit_attached_self));
        let parameter = if has_explicit_attached_self && index == 0 {
            None
        } else {
            parameters.get(parameter_index)
        };
        let dynamic_conformance = parameter.and_then(|parameter| {
            forwarded_dynamic_conformance(
                context,
                source_key,
                statement_index,
                *expression,
                parameter,
            )
        });
        output_arguments.append_to_span(
            &mut arguments,
            StateCallArgument {
                index,
                parameter_symbol: if has_explicit_attached_self && index == 0 {
                    target_key.machine
                } else {
                    parameter
                        .map(|parameter| parameter.symbol)
                        .unwrap_or_else(SymbolHandle::invalid)
                },
                parameter_name: if has_explicit_attached_self && index == 0 {
                    Identifier::generated_static("self")
                } else {
                    parameter
                        .map(|parameter| parameter.name.clone())
                        .unwrap_or_default()
                },
                expression: output_expressions
                    .copy_from(&context.control_flow.expressions, *expression),
                kind: if argument_is_mutable_alias(
                    context,
                    source_key,
                    statement_index,
                    role,
                    call_ordinal,
                    index,
                    *expression,
                ) || parameter.is_some_and(|parameter| parameter.is_mutable_reference)
                {
                    StateCallArgumentKind::MutableAlias
                } else {
                    StateCallArgumentKind::Value
                },
                dynamic_conformance,
                required,
            },
        );
    }

    arguments
}

fn forwarded_dynamic_conformance(
    context: &StateCallPlanningContext,
    source_key: StateKey,
    statement_index: usize,
    expression: ExpressionHandle,
    parameter: &StateParameterFlow,
) -> Option<StateCallDynamicConformance> {
    if !parameter.dyn_conformance_rows.is_empty() || parameter.dyn_conformance_candidates.is_empty()
    {
        return None;
    }
    let (binding, binding_name) =
        expression_binding(&context.control_flow.expressions, expression)?;
    let selection = context
        .control_flow
        .semantics
        .facts
        .dynamic_conformances
        .for_receiver(
            source_key.machine,
            source_key.state,
            binding,
            &binding_name,
            statement_index,
        )?;
    let conformance = selection.conformance?;
    let candidate = parameter
        .dyn_conformance_candidates
        .iter()
        .find(|candidate| {
            candidate.source_data == selection.source_data
                && candidate.conformance == Some(conformance)
                && candidate.rows == selection.rows
        })?;
    if selection.target_trait != parameter.type_symbol || candidate.rows.is_empty() {
        return None;
    }
    Some(StateCallDynamicConformance {
        source_binding: selection.binding,
        source_data: selection.source_data,
        target_trait: selection.target_trait,
        conformance,
        rows: selection.rows.clone(),
    })
}

fn expression_binding(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, Identifier)> {
    match expressions.expression(expression) {
        ExpressionNode::Borrow(inner) => expression_binding(expressions, inner.target),
        ExpressionNode::Name(path) => Some((
            path.symbol,
            expressions.name_path_members(path.members).last()?.clone(),
        )),
        _ => None,
    }
}

fn state_parameters(
    context: &StateCallPlanningContext,
    target_key: StateKey,
) -> &[StateParameterFlow] {
    state_flow_from_key(context, target_key)
        .map(|state| context.control_flow.state_parameters(state))
        .unwrap_or_default()
}

fn argument_is_mutable_alias(
    context: &StateCallPlanningContext,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
    index: usize,
    expression: ExpressionHandle,
) -> bool {
    if let Some(call) = context.borrow_call_by_key(source_key, statement_index, call_ordinal)
        && let Some(access) = context
            .control_flow
            .semantics
            .borrow
            .argument_accesses
            .span(call.accesses)
            .and_then(|accesses| accesses.get(index))
    {
        return access.kind == omega_control_flow::StateBorrowAccessKind::Mutable;
    }

    let _ = role;
    matches!(
        context.control_flow.expressions.expression(expression),
        ExpressionNode::Borrow(_)
    )
}

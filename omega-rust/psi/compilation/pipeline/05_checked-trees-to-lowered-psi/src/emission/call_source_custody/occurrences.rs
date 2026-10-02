//! Exact captured expression-call occurrences shared by result and Unit tails.
use super::{CheckedTrees, LoweringError, unsupported};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

/// Callers establish the authored destination and signature independently.
/// This join retains the captured outer occurrence without walking operands
/// again or assigning nested call ordinals from their evaluation order.
pub(crate) fn validate(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    expression: ExpressionHandle,
) -> Result<(), LoweringError> {
    if !checked.expression_table.expression_is_valid(expression) {
        return unsupported("captured outer call has no live authored expression");
    }
    let ExpressionNode::Call(authored) = checked.expression_table.expression(expression) else {
        return unsupported("computed result initializer has no authored call");
    };
    // An operator namespace is not a runtime receiver. Reconstruct the
    // declaration's operand count independently of the checked call row,
    // retaining both the exact target and the authored namespace stamp.
    let operator = typed_trees::operator::resolve_named_expression_call(&checked.typed, authored)
        .filter(|operator| operator.is_boundary);
    let target = operator.map_or(authored.target_symbol, |operator| operator.symbol);
    if operator.is_none() {
        if !authored.carries_only_positional_arguments() || !authored.selects_only_nominal_route() {
            return unsupported("captured call has no ordinary positional application");
        }
        let signature = super::authored::target_signature(checked, caller_machine, target)?;
        if !signature
            .parameters
            .iter()
            .any(|parameter| parameter.is_self)
            && !service_receiver_matches(
                checked,
                caller_machine,
                caller_state,
                target,
                authored.receiver,
            )
        {
            let receiver = if authored.receiver.is_valid()
                && checked
                    .expression_table
                    .expression_is_valid(authored.receiver)
            {
                let ExpressionNode::Name(path) =
                    checked.expression_table.expression(authored.receiver)
                else {
                    return unsupported(
                        "nonself call receiver is neither a namespace nor a service carrier",
                    );
                };
                path.symbol
            } else {
                symbols::SymbolHandle::invalid()
            };
            super::authored::nested::validate_static_receiver(
                checked,
                caller_machine,
                target,
                receiver,
                authored.receiver.is_valid(),
            )?;
        }
    }
    let has_receiver = authored.receiver.is_valid()
        && operator.is_none_or(|operator| {
            checked.operator_parameters(operator).len()
                == checked
                    .expression_table
                    .expression_handles(authored.arguments)
                    .len()
                    + 1
        });
    let receiver = if authored.receiver.is_valid() {
        if !checked
            .expression_table
            .expression_is_valid(authored.receiver)
        {
            return unsupported("captured outer call has no live receiver expression");
        }
        match checked.expression_table.expression(authored.receiver) {
            ExpressionNode::Indexed(_) | ExpressionNode::Member(_) => {
                super::projected_receivers::source(
                    checked,
                    caller_machine,
                    caller_state,
                    coordinate.statement_index as usize,
                    authored.receiver,
                )?
                .stamp
            }
            ExpressionNode::Name(path) if path.symbol.is_valid() => path.symbol,
            _ => return unsupported("captured outer call has no supported receiver identity"),
        }
    } else {
        symbols::SymbolHandle::invalid()
    };
    let control = &checked.facts.flow.control;
    let mut states = control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| {
            state.machine_symbol == caller_machine && state.state_symbol == caller_state
        });
    let state = states.next().ok_or(LoweringError::Unsupported(
        "computed result initializer has no checked state",
    ))?;
    if states.next().is_some() {
        return unsupported("computed result initializer has ambiguous checked state");
    }
    let calls = control
        .calls
        .span(state.calls)
        .ok_or(LoweringError::Unsupported(
            "computed result initializer has an invalid call span",
        ))?;
    let mut calls = calls.iter().filter(|call| {
        call.statement_index == coordinate.statement_index as usize
            && call.call_ordinal == coordinate.call_ordinal as usize
    });
    let call = calls.next().ok_or(LoweringError::Unsupported(
        "computed result initializer has no checked outer call",
    ))?;
    if calls.next().is_some()
        || call.authored_expression != expression
        || call.target_symbol != target
        || call.has_receiver != has_receiver
        || call.receiver_symbol != receiver
    {
        return unsupported(
            "computed result initializer disagrees with its captured outer occurrence",
        );
    }
    Ok(())
}

/// A routed service has receiver identity but no runtime self parameter.
/// Its exact carrier/requirement relation distinguishes it from a namespace;
/// the call operation still rejoins the Fused erasure and source-place custody.
fn service_receiver_matches(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    target: symbols::SymbolHandle,
    receiver: ExpressionHandle,
) -> bool {
    if !receiver.is_valid() || !checked.expression_table.expression_is_valid(receiver) {
        return false;
    }
    let Ok((machine, state)) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)
    else {
        return false;
    };
    if machine.symbol != caller_machine {
        return false;
    }
    let Some(reference) =
        validation::declared_place_type_raw(&checked.typed, machine, Some(state), receiver)
    else {
        return false;
    };
    let Ok(Some(carrier)) =
        typed_trees::service::classify_exact_service_parameter_carrier(&checked.typed, reference)
    else {
        return false;
    };
    checked.traits().iter().any(|definition| {
        definition.symbol == carrier.carrier.requirement
            && checked
                .trait_machine_signatures(definition)
                .iter()
                .any(|signature| signature.symbol == target)
    })
}

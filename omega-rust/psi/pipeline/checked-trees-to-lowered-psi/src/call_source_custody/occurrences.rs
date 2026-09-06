//! Exact captured expression-call occurrences shared by result and Unit tails.

use super::*;
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
    let receiver = if authored.receiver.is_valid() {
        if !checked
            .expression_table
            .expression_is_valid(authored.receiver)
        {
            return unsupported("captured outer call has no live receiver expression");
        }
        match checked.expression_table.expression(authored.receiver) {
            ExpressionNode::Name(path) if path.symbol.is_valid() => path.symbol,
            ExpressionNode::Member(member) if member.member_symbol.is_valid() => {
                let (machine, _) =
                    crate::scalar_source_custody::authored_state(checked, caller_state)?;
                if machine.symbol != caller_machine {
                    return unsupported("captured outer receiver has a different caller");
                }
                // Attached machines retain inherited field symbols. Rejoin
                // the exact declared storage field, as call capture does,
                // without recovering an absent or conflicting source stamp.
                validation::exact_self_field(&checked.typed, machine, authored.receiver)
                    .ok_or(LoweringError::Unsupported(
                        "captured outer receiver is not its exact attached self field",
                    ))?
                    .symbol
            }
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
        || call.target_symbol != authored.target_symbol
        || call.has_receiver != authored.receiver.is_valid()
        || call.receiver_symbol != receiver
    {
        return unsupported(
            "computed result initializer disagrees with its captured outer occurrence",
        );
    }
    Ok(())
}

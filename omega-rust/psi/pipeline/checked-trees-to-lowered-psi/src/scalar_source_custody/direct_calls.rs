//! Rejoin a direct call's outer declaration even when it has no arguments.

use super::*;

pub(super) struct DirectCall<'a> {
    source: SourceRoot,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    pub arguments: &'a [ExpressionHandle],
    pub parameters: &'a [checked_trees::signature::StateParameter],
}

pub(super) fn locate(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    binding_ordinal: u32,
) -> Result<DirectCall<'_>, LoweringError> {
    // This also checks immutability, initialized primitive carrier, and the
    // preceding declaration count without depending on an argument stamp.
    let source = super::locate(
        checked,
        state,
        statement,
        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
    )?;
    let program = &checked.typed;
    let ExpressionNode::Call(call) = program.expression_table.expression(source.expression) else {
        return unsupported("direct scalar binding has no authored call initializer");
    };
    if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
        return unsupported("direct scalar binding requires an authored free call");
    }
    let mut targets = program.machines().iter().filter_map(|machine| {
        program.machine_states(machine).first().and_then(|entry| {
            (machine.symbol.is_valid()
                && call.target_symbol.is_valid()
                && entry.symbol == call.target_symbol)
                .then_some((machine, entry))
        })
    });
    let (machine, entry) = targets.next().ok_or(LoweringError::Unsupported(
        "direct scalar binding has no authored callee entry",
    ))?;
    if targets.next().is_some() {
        return unsupported("direct scalar binding has ambiguous authored callee ownership");
    }
    let parameters = program.state_parameters(entry);
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.len() != parameters.len()
        || parameters.iter().any(|parameter| {
            parameter.is_self
                || parameter.is_const
                || parameter.is_mutable
                || program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        || program.primitive_type_reference(entry.return_type) != Some(source.primitive_type)
    {
        return unsupported("direct scalar binding disagrees with its authored callee signature");
    }
    Ok(DirectCall {
        source,
        target_machine: machine.symbol,
        target_state: entry.symbol,
        arguments,
        parameters,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    statement: u32,
    binding_ordinal: u32,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    call_ordinal: u32,
    argument_count: u32,
    result_type: ScalarType,
) -> Result<(), LoweringError> {
    let call = locate(checked, caller_state, statement, binding_ordinal)?;
    if call.source.machine != caller_machine
        || call.target_machine != target_machine
        || call.target_state != target_state
        || call_ordinal != 0
        || usize::try_from(argument_count).ok() != Some(call.arguments.len())
        || terminal_scalar_type(call.source.primitive_type)? != result_type
    {
        return unsupported("direct scalar call disagrees with its authored declaration or target");
    }
    Ok(())
}

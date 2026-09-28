use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;

pub(crate) fn retain_exact_flow_call(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    target: symbols::SymbolHandle,
) -> Result<&checked_trees::FlowCallFact, LoweringError> {
    let mut states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter_map(|(_, state)| {
            (state.machine_symbol == machine && state.state_symbol == source_state).then_some(state)
        });
    let Some(state) = states.next() else {
        return unsupported("Unit scalar call is missing its original checked flow state");
    };
    if states.next().is_some() {
        return unsupported("Unit scalar call has duplicate original checked flow states");
    }
    let statement_index = usize::try_from(coordinate.statement_index).map_err(|_| {
        LoweringError::Unsupported("Unit scalar call statement coordinate exceeds usize")
    })?;
    let call_ordinal = usize::try_from(coordinate.call_ordinal).map_err(|_| {
        LoweringError::Unsupported("Unit scalar call ordinal coordinate exceeds usize")
    })?;
    let authored = crate::emission::call_source_custody::authored::locate_source(
        checked,
        source_state,
        coordinate,
    )?;
    if authored.target_state != target {
        return unsupported("Unit result call disagrees with its authored resolved target");
    }
    let mut exact_calls = checked
        .facts
        .flow
        .control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .filter(|call| {
            call.statement_index == statement_index && call.call_ordinal == call_ordinal
        });
    // Flow retains the authored callable parameter, while the operation names
    // its resolved boundary requirement. Rejoin both identities through source.
    let exact = exact_calls.next().ok_or(LoweringError::Unsupported(
        "Unit call has no original checked flow occurrence",
    ))?;
    if exact.target_symbol != authored.source_target || exact_calls.next().is_some() {
        return unsupported(
            "Unit scalar call coordinate and target do not rejoin its original checked flow call",
        );
    }
    Ok(exact)
}

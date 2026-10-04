//! Rejoin bound, temporary, and discarded result sources independently of operands.
use super::{CheckedTrees, LoweringError};
use checked_trees::statement::StatementNode;

/// A discarded invocation produces a value but no source-local binding.
pub(crate) fn discards_result(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> Result<bool, LoweringError> {
    let (_, source) = crate::declarations::state_declaration(checked, state)?;
    let statement = checked
        .statement_table
        .statements(source.statement_nodes)
        .get(coordinate.statement_index as usize)
        .ok_or(LoweringError::Unsupported(
            "call result has no authored statement",
        ))?;
    Ok(coordinate.call_ordinal == 0
        && matches!(statement, StatementNode::Call(call) if call.discards_result))
}

/// Whether the statement-level call at `coordinate` initializes a declared
/// immutable local, so its retained result is a source-named value. A result
/// an assignment stores has no source name of its own.
pub(crate) fn initializes_immutable_local(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> Result<bool, LoweringError> {
    let (_, source) = crate::declarations::state_declaration(checked, state)?;
    let statement = checked
        .statement_table
        .statements(source.statement_nodes)
        .get(coordinate.statement_index as usize)
        .ok_or(LoweringError::Unsupported(
            "call result has no authored statement",
        ))?;
    let StatementNode::LocalData(local) = statement else {
        return Ok(false);
    };
    if local.is_mutable {
        return Ok(false);
    }
    let authored = super::authored::locate_source(checked, state, coordinate)?;
    // Preorder zero can also name the first nested call of a computation.
    // Only the initializer expression itself completes this source binding.
    Ok(authored.source_site
        == Some(checked_trees::NominalMachineUseSite::Expression(
            local.initial_value,
        )))
}

//! A selected owner retains its original alternatives, not a new establishment.
//! Rejoin receipts to actual declarations and occurrences before emission can
//! carry the selected owner and its ordered complement through a continuation.

use super::*;
use checked_trees::{
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    FlowOwnedSelectionReceipt,
};
use language_semantics::{
    Multiplicity, PermissionClaimIdentity, PermissionEventSource, PermissionProvenance,
};

pub(super) fn validate_receipt(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
    receipt: &FlowOwnedSelectionReceipt,
) -> Result<(), LoweringError> {
    let (owner, authored) = crate::scalar_source_custody::authored_state(checked, state)?;
    let statements = checked.statement_table.statements(authored.statement_nodes);
    let (expression, destination, reference) = match statements.get(statement as usize) {
        Some(StatementNode::LocalData(local)) if !local.is_mutable => {
            (local.initial_value, local.symbol, local.type_reference)
        }
        Some(StatementNode::Expression(expression))
            if statement as usize + 1 == statements.len() =>
        {
            (*expression, SymbolHandle::invalid(), authored.return_type)
        }
        _ => return unsupported("selected ownership has no exact value destination"),
    };
    if owner.symbol != machine
        || receipt.machine != machine
        || receipt.state != state
        || receipt.statement_ordinal != statement
        || receipt.expression != expression
        || receipt.destination != destination
        || receipt.type_reference != reference
        || receipt.death != PermissionEventSource::StateExit
        || checked.type_multiplicity(reference) != Multiplicity::Affine
        || !validation::has_plain_owned_contents(&checked.typed, reference)
        || !matches!(
            checked.expression_table.expression(expression),
            ExpressionNode::Match(_)
        )
    {
        return unsupported("selected ownership substituted its destination or death edge");
    }
    let ownership = &checked.facts.flow.ownership;
    let sources =
        ownership
            .selection_sources
            .span(receipt.sources)
            .ok_or(LoweringError::Unsupported(
                "selected ownership has a stale source span",
            ))?;
    let transfers = ownership
        .selection_transfers
        .span(receipt.transfers)
        .ok_or(LoweringError::Unsupported(
            "selected ownership has a stale transfer span",
        ))?;
    if sources.is_empty() || transfers.is_empty() {
        return unsupported("selected ownership omitted its source alternatives");
    }
    let mut previous = statement;
    for (ordinal, source) in sources.iter().enumerate() {
        let Some(StatementNode::LocalData(local)) =
            statements.get(source.statement_ordinal as usize)
        else {
            return unsupported("selected ownership source has no local declaration");
        };
        if source.statement_ordinal >= previous
            || source.symbol != local.symbol
            || local.is_mutable
            || !local.initial_value.is_valid()
            || checked.normalized_type_identity(local.type_reference)
                != checked.normalized_type_identity(reference)
            || source.claim_identity != PermissionClaimIdentity::Unknown
            || sources[..ordinal]
                .iter()
                .any(|prior| prior.symbol == source.symbol)
        {
            return unsupported("selected ownership changed its exact source or declaration order");
        }
        previous = source.statement_ordinal;
        if source.origin_selection.is_valid() {
            if !ownership.owned_selections.is_valid(source.origin_selection)
                || ownership
                    .owned_selection_at(state, source.statement_ordinal)
                    .is_none_or(|(handle, _)| handle != source.origin_selection)
            {
                return unsupported("selected ownership lost its prior selection origin");
            }
            let prior = ownership.owned_selections.get(source.origin_selection);
            if prior.machine != machine
                || prior.state != state
                || prior.statement_ordinal != source.statement_ordinal
                || prior.destination != source.symbol
                || source.provenance != PermissionProvenance::Unknown
            {
                return unsupported("selected ownership substituted its prior result origin");
            }
        } else if source.provenance
            != (PermissionProvenance::Established {
                machine_symbol: machine,
                state_symbol: state,
                source: PermissionEventSource::Statement {
                    statement_index: source.statement_ordinal as usize,
                },
            })
            || ownership
                .owned_selection_at(state, source.statement_ordinal)
                .is_some()
        {
            return unsupported("selected ownership minted or lost source provenance");
        }
        let source_handle =
            arena::Handle::from_parts(
                receipt
                    .sources
                    .start()
                    .arena_index()
                    .checked_add(u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("selected source ordinal overflow")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "selected source handle overflow",
                    ))?,
                receipt.sources.start().generation(),
            );
        if !transfers
            .iter()
            .any(|transfer| transfer.source == source_handle)
        {
            return unsupported("selected ownership added an unauthored residual owner");
        }
    }
    for (ordinal, transfer) in transfers.iter().enumerate() {
        if !ownership.selection_sources.is_valid(transfer.source)
            || transfer.source.generation() != receipt.sources.start().generation()
            || transfer
                .source
                .arena_index()
                .checked_sub(receipt.sources.start().arena_index())
                .is_none_or(|offset| offset as usize >= sources.len())
            || !transfer.source_arm.is_valid()
            || transfers[..ordinal]
                .iter()
                .any(|prior| prior.expression == transfer.expression)
        {
            return unsupported("selected ownership reused or substituted a transfer occurrence");
        }
        let source = ownership.selection_sources.get(transfer.source);
        if validation::plain_owned_value_source(&checked.typed, transfer.expression, reference)
            != Some(source.symbol)
        {
            return unsupported("selected ownership changed the authored source place");
        }
    }
    Ok(())
}

pub(super) fn validate_leaf(
    checked: &CheckedTrees,
    receipt: &FlowOwnedSelectionReceipt,
    expression: ExpressionHandle,
    source_arm: arena::Handle<checked_trees::expression::TableMatchArm>,
    place: &CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let ownership = &checked.facts.flow.ownership;
    let transfer = ownership
        .owned_selection_transfer(receipt, expression)
        .ok_or(LoweringError::Unsupported(
            "selected place has no unique transfer receipt",
        ))?;
    if !ownership.selection_sources.is_valid(transfer.source) {
        return unsupported("selected place has a stale source owner");
    }
    let source = ownership.selection_sources.get(transfer.source);
    if transfer.source_arm != source_arm
        || place.source
            != (CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: source.symbol,
            })
        || !place.path.is_empty()
        || place.access != checked_trees::CheckedStructuralAccess::Owned
        || place.type_identity
            != checked
                .normalized_type_identity(receipt.type_reference)
                .as_str()
    {
        return unsupported("selected place changed its transfer, type or access");
    }
    Ok(())
}

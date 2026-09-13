//! Local receiver loans retain the established result's original storage.

use super::*;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitCallCoordinate, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralParameterPlan,
};

pub(super) fn declaration<'a>(
    checked: &'a CheckedTrees,
    state: &checked_trees::state::State,
    statement: usize,
    symbol: SymbolHandle,
) -> Option<(usize, &'a checked_trees::statement::TableLocalData)> {
    let mut declarations = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement)
        .enumerate()
        .filter_map(|(ordinal, statement)| match statement {
            checked_trees::statement::StatementNode::LocalData(local) if local.symbol == symbol => {
                Some((ordinal, local))
            }
            _ => None,
        });
    let (ordinal, local) = declarations.next()?;
    (declarations.next().is_none()
        && local.initial_value.is_valid()
        && validation::has_plain_owned_contents_with_numeric_constraints(
            &checked.typed,
            local.type_reference,
        ))
    .then_some((ordinal, local))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    caller_operations: &[CheckedUnitEffectOperationPlan],
    coordinate: CheckedUnitCallCoordinate,
    target_symbol: SymbolHandle,
    target: &CheckedUnitStructuralParameterPlan,
    argument: &CheckedUnitStructuralArgumentPlan,
    source: &ReceiverSource,
) -> Result<(), LoweringError> {
    let (_, state) = crate::scalar_source_custody::authored_state(checked, caller_state)?;
    let (ordinal, local) = declaration(
        checked,
        state,
        coordinate.statement_index as usize,
        source.root,
    )
    .ok_or(LoweringError::Unsupported(
        "local receiver declaration missing",
    ))?;
    let binding =
        argument
            .source_structural_result_binding_ordinal()
            .ok_or(LoweringError::Unsupported(
                "local receiver does not name established result storage",
            ))?;
    let mut producers = caller_operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
            | CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
                if result.binding_ordinal == binding =>
            {
                Some(result)
            }
            _ => None,
        });
    let result = producers.next().ok_or(LoweringError::Unsupported(
        "local receiver producer missing",
    ))?;
    let referent = if source.path.is_empty() {
        local.type_reference
    } else {
        checked
            .data_definitions()
            .iter()
            .flat_map(|data| checked.data_members(data))
            .find_map(|member| match member {
                checked_trees::data::DataMember::Field(field) if field.symbol == source.stamp => {
                    Some(field.type_reference)
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "local receiver projected type missing",
            ))?
    };
    if producers.next().is_some()
        || result.statement_index as usize != ordinal
        || result.type_identity
            != checked
                .normalized_type_identity(local.type_reference)
                .as_str()
        || result.multiplicity != checked.type_multiplicity(local.type_reference)
        || !matches!(
            result.multiplicity,
            language_semantics::Multiplicity::Affine
                | language_semantics::Multiplicity::Unrestricted
        )
        || !target.is_self
        || !target.qualifications.is_empty()
        || source.erased_alias
        || argument.path != source.path
        || argument.type_identity != target.type_identity
        || argument.access != target.access
        || checked.normalized_type_identity(referent).as_str() != target.type_identity
        || !matches!(
            argument.access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        )
        || (argument.access == CheckedStructuralAccess::MutableBorrow && !local.is_mutable)
    {
        return unsupported("local receiver loan substituted its producer, type, path, or access");
    }
    let borrow = &checked.facts.borrow;
    let state = borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.machine_symbol == caller_machine && state.state_symbol == caller_state)
        .ok_or(LoweringError::Unsupported(
            "local receiver borrow state missing",
        ))?;
    let mut calls = borrow
        .calls
        .span_or_empty(state.calls)
        .iter()
        .filter(|call| {
            call.statement_index == coordinate.statement_index as usize
                && call.call_ordinal == coordinate.call_ordinal as usize
                && call.target_symbol == target_symbol
        });
    let call = calls.next().ok_or(LoweringError::Unsupported(
        "local receiver borrow occurrence missing",
    ))?;
    // The source reader above independently reconstructs the receiver path;
    // implicit receiver capture is separate from explicit argument accesses.
    if calls.next().is_some() || !call.has_receiver || call.receiver_symbol != source.stamp {
        return unsupported("local receiver lacks its exact captured call receiver");
    }
    Ok(())
}

//! One retained value dependency roster for anonymous owner dispositions.
//! Source checking owns eligibility and the exact call coordinate; the value
//! graph identifies already-produced homes, independently of descriptor shape.
use super::actions::disposition_actions as selected_regions;
use crate::execution::terminal_unit::types::ShapeCollector;
use checked_trees::{
    CheckFacts, CheckedNormalCleanupAction, CheckedUnitCallCoordinate, CheckedUnitCallableEntry,
    CheckedUnitStructuralArgumentSourcePlan,
};
use language_semantics::PermissionEventSource;
use symbols::SymbolHandle;
pub(super) fn selected_actions(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    target: SymbolHandle,
    root: facts::PlaceRoot,
    source: CheckedUnitStructuralArgumentSourcePlan,
) -> Option<Vec<CheckedNormalCleanupAction>> {
    let mut actions = Vec::new();
    let mut retirement = None;
    for disposition_handle in facts
        .contextual_cleanup
        .state_disposition_handles(machine, state)
    {
        let disposition = facts
            .contextual_cleanup
            .dispositions
            .get(disposition_handle);
        if disposition.root != root
            || !matches!(disposition.source, PermissionEventSource::Call { statement_index, call_ordinal, target_symbol }
                if target_symbol == target && statement_index == coordinate.statement_index as usize
                    && call_ordinal == coordinate.call_ordinal as usize)
        {
            continue;
        }
        if retirement.is_none() {
            retirement = Some(super::actions::home_retirement(
                program,
                shapes,
                disposition,
                source.clone(),
            )?);
        }
        actions.extend(
            selected_regions(
                program,
                facts,
                targets,
                shapes,
                disposition_handle,
                source.clone(),
                None,
            )
            .ok()?,
        );
    }
    if let Some(retirement) = retirement {
        super::actions::finish_home(&mut actions, retirement);
    }
    Some(actions)
}

pub(in crate::execution::terminal_unit) fn dependency_dispositions(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    root: checked_trees::CheckedStructuralValueHandle,
) -> Option<Vec<arena::Handle<checked_trees::ContextualCleanupDisposition>>> {
    let mut selected = Vec::new();
    for value in facts.values.structural_values.value_dependencies(root)? {
        if value == root {
            continue;
        }
        let owner = facts::PlaceRoot::Expression(
            facts.values.structural_values.nodes.get(value).expression,
        );
        for (handle, row) in facts
            .contextual_cleanup
            .dispositions
            .iter()
            .filter(|(_, row)| {
                row.machine == machine
                    && row.state == state
                    && row.root == owner
                    && matches!(row.source, PermissionEventSource::Call { .. })
            })
        {
            if !selected.iter().any(|handle| {
                let existing = facts.contextual_cleanup.dispositions.get(*handle);
                existing.root == row.root && existing.source == row.source
            }) {
                selected.push(handle);
            }
        }
    }
    Some(selected)
}

pub(super) fn owner_group_actions(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    disposition: arena::Handle<checked_trees::ContextualCleanupDisposition>,
    source: CheckedUnitStructuralArgumentSourcePlan,
) -> Option<Vec<CheckedNormalCleanupAction>> {
    let row = facts.contextual_cleanup.dispositions.get(disposition);
    let PermissionEventSource::Call {
        statement_index,
        call_ordinal,
        target_symbol,
    } = row.source
    else {
        return None;
    };
    selected_actions(
        program,
        facts,
        targets,
        shapes,
        row.machine,
        row.state,
        CheckedUnitCallCoordinate {
            statement_index: u32::try_from(statement_index).ok()?,
            call_ordinal: u32::try_from(call_ordinal).ok()?,
        },
        target_symbol,
        row.root,
        source,
    )
}

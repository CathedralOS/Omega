//! Complete a data initializer's lexical owner using its checked complement.
use super::temporary_actions::SelectedActions;
use crate::execution::terminal_unit::types::ShapeCollector;
use checked_trees::{
    CheckFacts, CheckedUnitCallableEntry, CheckedUnitStructuralArgumentSourcePlan,
};
use symbols::SymbolHandle;

pub(super) fn actions(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
) -> Option<SelectedActions> {
    let owner = validation::data_bindings::completed_owner(
        program,
        program.state_by_symbol(state)?,
        statement,
    )?;
    let source = CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: owner };
    let mut actions = Vec::new();
    let mut retirement = None;
    let mut selected_regions = Vec::new();
    for handle in facts
        .contextual_cleanup
        .state_disposition_handles(machine, state)
    {
        let disposition = facts.contextual_cleanup.dispositions.get(handle);
        if disposition.root != facts::PlaceRoot::Symbol(owner)
            || disposition.source
                != (language_semantics::PermissionEventSource::Statement {
                    statement_index: statement,
                })
        {
            continue;
        }
        if facts.contextual_cleanup.is_selected_disposition(handle) {
            selected_regions.push(super::actions::selected_region(
                program,
                facts,
                shapes,
                handle,
                source.clone(),
                None,
            )?);
        }
        actions.extend(
            super::actions::disposition_actions(
                program,
                facts,
                targets,
                shapes,
                handle,
                source.clone(),
                None,
            )
            .ok()?,
        );
        retirement = Some(super::actions::home_retirement(
            program,
            shapes,
            disposition,
            source.clone(),
        )?);
    }
    if let Some(retirement) = retirement {
        super::actions::finish_home(&mut actions, retirement);
    }
    Some(SelectedActions {
        actions,
        selected_regions,
    })
}

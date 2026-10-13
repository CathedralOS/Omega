//! Selected normal-edge custody, expressed in the ordinary state value namespace.
//! The shared ownership selection supplies order and exact outgoing coordinates;
//! hook requirements remain obligations at that edge, never new unconditional facts.
use crate::execution::terminal_unit::types::ShapeCollector;
use checked_trees::{
    CheckFacts, CheckedComposedUnitControlStatePlan, CheckedNormalExitCleanupPlan,
    CheckedUnitCallableEntry, CheckedUnitStructuralArgumentSourcePlan,
};
use facts::PlaceRoot;
use language_semantics::PermissionEventSource;
use symbols::SymbolHandle;

pub(in crate::execution::terminal_unit) fn state_cleanups(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    state_plan: &CheckedComposedUnitControlStatePlan,
) -> Option<Vec<CheckedNormalExitCleanupPlan>> {
    let (machine, state) = program.machines().iter().find_map(|machine| {
        program
            .state_in_machine(machine, state_plan.state)
            .map(|state| (machine, state))
    })?;
    cleanup_rows(
        program,
        facts,
        targets,
        shapes,
        machine.symbol,
        state,
        |root| owner_source(program, state, state_plan, root).map(Some),
    )
}

/// Sequence completion disposes current named storage through the same checked
/// exits as state graphs. Anonymous evaluation results keep their own producer
/// disposition; named locals must never revive an overwritten birth result.
pub(in crate::execution::terminal_unit) fn sequence_cleanups(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    plan: &checked_trees::CheckedUnitEffectMachinePlan,
) -> Option<Vec<CheckedNormalExitCleanupPlan>> {
    let machine = program.machine_by_symbol(plan.machine)?;
    let state = program.state_in_machine(machine, plan.state)?;
    cleanup_rows(
        program,
        facts,
        targets,
        shapes,
        plan.machine,
        state,
        |root| {
            let position = program
                .state_parameters(state)
                .iter()
                .position(|parameter| root == PlaceRoot::Symbol(parameter.symbol));
            let Some(position) = position else {
                return Some(super::super::local_storage::source(
                    program,
                    state,
                    &plan.operations,
                    root.as_symbol()?,
                ));
            };
            let Some(index) = plan
                .structural_parameters
                .iter()
                .position(|parameter| parameter.position as usize == position)
            else {
                return Some(None);
            };
            Some(Some(CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(index).ok()?,
            }))
        },
    )
}

fn cleanup_rows(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    mut source_for_owner: impl FnMut(
        PlaceRoot,
    ) -> Option<Option<CheckedUnitStructuralArgumentSourcePlan>>,
) -> Option<Vec<CheckedNormalExitCleanupPlan>> {
    let flow_state = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, row)| (row.state_symbol == state.symbol).then_some(row))?;
    let mut rows = Vec::<CheckedNormalExitCleanupPlan>::new();
    for outgoing in facts
        .flow
        .control
        .outgoing
        .span_or_empty(flow_state.outgoing)
    {
        let kind = if !outgoing.transition_target.is_valid() {
            checked_trees::FlowClaimJoinExitKind::FinalExpression
        } else {
            let transition = program
                .statement_table
                .statement_at(state, outgoing.statement_index)?
                .as_transition()?;
            if transition.target == outgoing.transition_target {
                checked_trees::FlowClaimJoinExitKind::TransitionTarget
            } else if transition.continuation == outgoing.transition_target {
                checked_trees::FlowClaimJoinExitKind::TransitionContinuation
            } else {
                return None;
            }
        };
        let exit = checked_trees::FlowClaimJoinExit {
            state_symbol: state.symbol,
            statement_index: outgoing.statement_index,
            kind,
        };
        if !rows.iter().any(|row| row.exit == exit) {
            rows.push(CheckedNormalExitCleanupPlan {
                exit,
                selected_regions: Vec::new(),
                actions: Vec::new(),
            });
        }
    }
    let mut retirements = Vec::new();
    let mut establishment_ordered = Vec::new();
    for (disposition_handle, disposition) in
        facts
            .contextual_cleanup
            .dispositions
            .iter()
            .filter(|(_, row)| {
                row.machine == machine
                    && row.state == state.symbol
                    && row.source == PermissionEventSource::StateExit
            })
    {
        if disposition.exit.state_symbol != state.symbol {
            return None;
        }
        let Some(source) = source_for_owner(disposition.root)? else {
            continue;
        };
        if disposition.establishment_ordered
            && !establishment_ordered.contains(&(disposition.exit, disposition.root))
        {
            establishment_ordered.push((disposition.exit, disposition.root));
        }
        if !retirements.iter().any(
            |(exit, retirement, _): &(_, checked_trees::CheckedNormalCleanupAction, _)| {
                *exit == disposition.exit && retirement.source() == &source
            },
        ) {
            retirements.push((
                disposition.exit,
                super::actions::home_retirement(program, shapes, disposition, source.clone())?,
                disposition.root,
            ));
        }
        let actions = super::actions::disposition_actions(
            program,
            facts,
            targets,
            shapes,
            disposition_handle,
            source.clone(),
            None,
        )?;
        if let Some(row) = rows.iter_mut().find(|row| row.exit == disposition.exit) {
            if facts
                .contextual_cleanup
                .selected_dispositions
                .iter()
                .any(|(_, candidate)| *candidate == disposition_handle)
            {
                let region = super::actions::selected_region(
                    program,
                    facts,
                    shapes,
                    disposition_handle,
                    source.clone(),
                    None,
                )?;
                if !row.selected_regions.contains(&region) {
                    row.selected_regions.push(region);
                }
            }
            row.actions.extend(actions);
        } else {
            return None;
        }
    }
    for (exit, retirement, root) in retirements {
        let row = rows.iter_mut().find(|row| row.exit == exit)?;
        // A root still under construction keeps the producer's reverse
        // establishment order; the declared order below is for a completed
        // aggregate whose candidate regions join its residual partition.
        if !establishment_ordered.contains(&(exit, root))
            && row
                .selected_regions
                .iter()
                .any(|region| &region.source == retirement.source() && !region.path.is_empty())
        {
            super::order::merge(
                program,
                state.symbol,
                exit.statement_index,
                root,
                &mut row.actions,
                retirement.source(),
            )?;
        }
        super::actions::finish_home(&mut row.actions, retirement);
    }
    Some(rows)
}

fn owner_source(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    plan: &CheckedComposedUnitControlStatePlan,
    root: PlaceRoot,
) -> Option<CheckedUnitStructuralArgumentSourcePlan> {
    let symbol = root.as_symbol()?;
    if let Some(position) =
        typed_trees::signature::state_parameter_position(program.state_parameters(state), symbol)
    {
        let index = plan
            .structural_parameters
            .iter()
            .position(|parameter| parameter.position as usize == position)?;
        return Some(CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(index).ok()?,
        });
    }
    super::super::local_storage::source(program, state, &plan.operations, symbol)
}

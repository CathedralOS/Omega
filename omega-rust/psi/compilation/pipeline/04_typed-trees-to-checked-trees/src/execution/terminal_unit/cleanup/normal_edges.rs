//! Selected normal-edge custody, expressed in the ordinary state value namespace.
//! The shared ownership selection supplies order and exact outgoing coordinates;
//! hook requirements remain obligations at that edge, never new unconditional facts.
use crate::execution::terminal_unit::types::ShapeCollector;
use checked_trees::{
    CheckFacts, CheckedComposedUnitControlStatePlan, CheckedNormalExitCleanupPlan,
    CheckedUnitCallableEntry, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use facts::PlaceRoot;
use language_semantics::PermissionEventSource;
use typed_trees::{TypedTrees, statement::StatementNode};

pub(in crate::execution::terminal_unit) fn state_cleanups(
    program: &TypedTrees,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    state_plan: &CheckedComposedUnitControlStatePlan,
) -> Option<Vec<CheckedNormalExitCleanupPlan>> {
    let (machine, state) = program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_plan.state)
            .map(|state| (machine, state))
    })?;
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
            let StatementNode::Transition(transition) = program
                .statement_table
                .statements(state.statement_nodes)
                .get(outgoing.statement_index)?
            else {
                return None;
            };
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
                actions: Vec::new(),
            });
        }
    }
    let reference_consumed = crate::execution::terminal_unit::types::reference_consumed_parameters(
        &state_plan.operations,
    );
    for (_, disposition) in facts
        .contextual_cleanup
        .dispositions
        .iter()
        .filter(|(_, row)| {
            row.machine == machine.symbol
                && row.state == state.symbol
                && row.source == PermissionEventSource::StateExit
        })
    {
        if disposition.exit.state_symbol != state.symbol {
            return None;
        }
        let source = owner_source(program, state, state_plan, disposition.root)?;
        if matches!(source, CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index }
            if reference_consumed.contains(&parameter_index))
        {
            continue;
        }
        let actions = super::actions::disposition_actions(
            program,
            facts,
            targets,
            shapes,
            disposition,
            source,
            None,
        )?;
        if let Some(row) = rows.iter_mut().find(|row| row.exit == disposition.exit) {
            row.actions.extend(actions);
        } else {
            return None;
        }
    }
    Some(rows)
}

fn owner_source(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    plan: &CheckedComposedUnitControlStatePlan,
    root: PlaceRoot,
) -> Option<CheckedUnitStructuralArgumentSourcePlan> {
    let PlaceRoot::Symbol(symbol) = root else {
        return None;
    };
    if let Some(position) = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == symbol)
    {
        let index = plan
            .structural_parameters
            .iter()
            .position(|parameter| parameter.position as usize == position)?;
        return Some(CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(index).ok()?,
        });
    }
    for operation in plan.operations.iter().rev() {
        if let CheckedUnitEffectOperationPlan::RebindStructuralLocal { destination, .. } = operation
            && destination.source
                == (CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol })
        {
            // Rebinding publishes the local's new home; the initial producer
            // remains the identity of its already retired occurrence.
            return Some(destination.source.clone());
        }
        if matches!(operation, CheckedUnitEffectOperationPlan::EstablishStructuralLocal { symbol: local, .. } if *local == symbol)
        {
            return Some(CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol });
        }
        if let Some(result) = operation.statement_structural_result_binding()
            && matches!(program.statement_table.statements(state.statement_nodes).get(result.statement_index as usize),
                Some(StatementNode::LocalData(local)) if local.symbol == symbol)
        {
            return Some(CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            });
        }
    }
    None
}

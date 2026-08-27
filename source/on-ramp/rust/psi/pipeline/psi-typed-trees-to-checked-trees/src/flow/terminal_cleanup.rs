use std::collections::BTreeSet;

use psi_checked_trees::{
    CheckFacts, CheckedStructuralControlCleanupPlans, CheckedStructuralControlEdgeCleanupPlan,
    CheckedStructuralControlStateCleanupPlan,
};
use psi_language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use psi_typed_trees::{
    TypedTrees,
    statement::{StatementNode, TransitionExit, TransitionTargetNode},
};

use super::FlowOwnershipEventSource;

/// Retain the first source-handle-free structural control-edge cleanup slice.
///
/// The permission ledger is authoritative for which whole affine parameters
/// are eligible for no-code state-exit disposal. Per-arm move discovery is the
/// same checked ownership input used by multiplicity validation. Anything that
/// needs local, projected, nominal, or claim-bearing cleanup omits the complete
/// state plan rather than publishing partial evidence.
pub(crate) fn build_checked_structural_control_cleanup_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedStructuralControlCleanupPlans {
    let mut states = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if let Some(plan) = build_state_plan(program, facts, machine, state) {
                states.push(plan);
            }
        }
    }
    CheckedStructuralControlCleanupPlans { states }
}

fn build_state_plan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
) -> Option<CheckedStructuralControlStateCleanupPlan> {
    let parameters = program.state_parameters(state);
    let discard_parameters =
        checked_whole_affine_discard_parameters(program, facts, machine.symbol, state)?;

    let statements = program.statement_table.statements(state.statement_nodes);
    let has_structural_control = statements.iter().any(|statement| {
        let StatementNode::Transition(transition) = statement else {
            return false;
        };
        transition.exit == TransitionExit::Ordinary
            && matches!(
                program.statement_table.transition_target(transition.target),
                TransitionTargetNode::Named { path, .. }
                    if program
                        .machine_states(machine)
                        .iter()
                        .any(|target| target.symbol == path.symbol)
            )
    });
    if !has_structural_control {
        return None;
    }

    let mut segments = facts.flow.ownership.segments.clone();
    let moves =
        super::discover_state_move_events(program, &facts.borrow, machine, state, &mut segments);
    let mut edges = Vec::new();
    for (statement_index, statement) in statements.iter().enumerate() {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        if transition.exit != TransitionExit::Ordinary {
            continue;
        }
        let TransitionTargetNode::Named { path, .. } =
            program.statement_table.transition_target(transition.target)
        else {
            continue;
        };
        if !program
            .machine_states(machine)
            .iter()
            .any(|target| target.symbol == path.symbol)
        {
            continue;
        }

        let mut transferred = BTreeSet::new();
        for event in moves.iter().filter(|event| {
            matches!(
                event.source,
                FlowOwnershipEventSource::Statement {
                    statement_index: source_index,
                } if source_index == statement_index
            ) || matches!(
                event.source,
                FlowOwnershipEventSource::Call {
                    statement_index: source_index,
                    target_symbol,
                    ..
                } if source_index == statement_index && target_symbol == path.symbol
            )
        }) {
            let psi_facts::PlaceRoot::Symbol(root) = event.root else {
                continue;
            };
            let Some((_, position)) = discard_parameters
                .iter()
                .find(|(candidate, _)| *candidate == root)
            else {
                continue;
            };
            if !segments.span_or_empty(event.segments).is_empty() {
                return None;
            }
            transferred.insert(*position);
        }
        let trivial_affine_discard_parameter_positions = discard_parameters
            .iter()
            .filter_map(|(symbol, position)| {
                (!transferred.contains(position)).then_some((*symbol, *position))
            })
            .map(|(symbol, position)| {
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)?;
                (!super::terminal_unit::types::type_graph_requires_nominal_drop(
                    program,
                    parameter.type_reference,
                ))
                .then_some(position)
            })
            .collect::<Option<Vec<_>>>()?;
        edges.push(CheckedStructuralControlEdgeCleanupPlan {
            statement_ordinal: u32::try_from(statement_index).ok()?,
            target_state: path.symbol,
            trivial_affine_discard_parameter_positions,
        });
    }
    (!edges.is_empty()).then_some(CheckedStructuralControlStateCleanupPlan {
        machine: machine.symbol,
        state: state.symbol,
        edges,
    })
}

pub(super) fn checked_whole_affine_discard_parameters(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: psi_symbols::SymbolHandle,
    state: &psi_typed_trees::state::State,
) -> Option<Vec<(psi_symbols::SymbolHandle, u32)>> {
    let parameters = program.state_parameters(state);
    let entry_claim_roots = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.machine_symbol == machine
                && event.state_symbol == state.symbol
                && event.source == PermissionEventSource::StateEntry
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned)
                .then_some(event.root)
        })
        .collect::<Vec<_>>();
    let expected_discard_parameters = parameters
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(position, parameter)| {
            if parameter.is_self
                || crate::checks::type_multiplicity(program, parameter.type_reference)
                    != Multiplicity::Affine
                || entry_claim_roots.contains(&psi_facts::PlaceRoot::Symbol(parameter.symbol))
            {
                return None;
            }
            Some((parameter.symbol, u32::try_from(position).ok()?))
        })
        .collect::<Vec<_>>();
    let mut discard_parameters = Vec::new();
    for (_, event) in facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state.symbol
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
        })
    {
        if event.access != PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Affine
            || event.obligation_live
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != PermissionProvenance::Unknown
            || !facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
                .is_empty()
        {
            return None;
        }
        let psi_facts::PlaceRoot::Symbol(root) = event.root else {
            return None;
        };
        let position = parameters.iter().position(|parameter| {
            !parameter.is_self
                && parameter.symbol == root
                && crate::checks::type_multiplicity(program, parameter.type_reference)
                    == Multiplicity::Affine
        })?;
        let position = u32::try_from(position).ok()?;
        if discard_parameters.contains(&(root, position)) {
            return None;
        }
        discard_parameters.push((root, position));
    }
    if discard_parameters != expected_discard_parameters {
        return None;
    }
    Some(discard_parameters)
}

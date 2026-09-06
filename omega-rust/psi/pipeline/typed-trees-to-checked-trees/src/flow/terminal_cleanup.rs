use std::collections::BTreeSet;

use checked_trees::{
    CheckFacts, CheckedStructuralControlCleanupPlans, CheckedStructuralControlEdgeCleanupPlan,
    CheckedStructuralControlProjectedEdgeCleanupPlan,
    CheckedStructuralControlProjectedTransferPlan, CheckedStructuralControlStateCleanupPlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralPathSegment,
};
use language_semantics::{
    MachineSupplyMode, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, PermissionProvenance,
};
use typed_trees::{
    TypedTrees,
    statement::{StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode},
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
    let mut projected_edges = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if let Some(edge) = build_projected_edge_plan(program, facts, machine, state) {
                projected_edges.push(edge);
            }
            if let Some(plan) = build_state_plan(program, facts, machine, state) {
                states.push(plan);
            }
        }
    }
    CheckedStructuralControlCleanupPlans {
        states,
        projected_edges,
    }
}

fn build_projected_edge_plan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<CheckedStructuralControlProjectedEdgeCleanupPlan> {
    let states = program.machine_states(machine);
    if machine.supply_mode != MachineSupplyMode::CheckedBody
        || machine.attached_data.is_none()
        || states.len() != 2
        || !program.machine_contracts(machine).is_empty()
        || !program.state_contracts(state).is_empty()
        || !super::terminal_unit::cleanup_type_is_unit(program, state.return_type)
    {
        return None;
    }
    let [source_parameter] = program.state_parameters(state) else {
        return None;
    };
    if source_parameter.is_self {
        return None;
    }
    let [StatementNode::Transition(transition)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        return None;
    };
    if transition.exit != TransitionExit::Ordinary
        || transition.guard != TransitionGuardNode::Always
        || transition.continuation.is_valid()
    {
        return None;
    }
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    let target = states.iter().find(|target| target.symbol == path.symbol)?;
    if target.symbol == state.symbol
        || !program.state_contracts(target).is_empty()
        || !super::terminal_unit::cleanup_type_is_unit(program, target.return_type)
    {
        return None;
    }
    let [target_parameter] = program.state_parameters(target) else {
        return None;
    };
    if target_parameter.is_self {
        return None;
    }
    let [argument] = program.statement_table.expression_handles(*arguments) else {
        return None;
    };
    let argument_place =
        super::canonical_place_from_expression_in_state(program, state.symbol, 0, *argument)?;
    let facts::PlaceRoot::Symbol(argument_root) = argument_place.root else {
        return None;
    };
    let [
        facts::PlaceSegment::Field {
            symbol: moved_field,
        },
    ] = argument_place.segments.as_slice()
    else {
        return None;
    };
    if argument_root != source_parameter.symbol {
        return None;
    }

    let discard_parameters =
        checked_whole_affine_discard_parameters(program, facts, machine.symbol, state)?;
    if discard_parameters != [(source_parameter.symbol, 0)]
        || facts.flow.ownership.permissions.iter().any(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.claim_identity != PermissionClaimIdentity::Unknown
        })
    {
        return None;
    }
    let mut segments = facts.flow.ownership.segments.clone();
    let moves =
        super::discover_state_move_events(program, &facts.borrow, machine, state, &mut segments);
    let edge_moves = moves
        .iter()
        .filter(|event| {
            matches!(
                event.source,
                FlowOwnershipEventSource::Statement { statement_index: 0 }
            ) || matches!(
                event.source,
                FlowOwnershipEventSource::Call {
                    statement_index: 0,
                    target_symbol,
                    ..
                } if target_symbol == path.symbol
            )
        })
        .collect::<Vec<_>>();
    let [edge_move] = edge_moves.as_slice() else {
        return None;
    };
    if edge_move.root != facts::PlaceRoot::Symbol(source_parameter.symbol)
        || segments.span_or_empty(edge_move.segments) != argument_place.segments
    {
        return None;
    }

    let (
        moved_field_identity,
        moved_type_identity,
        residual_field_identity,
        residual_type_identity,
    ) = super::terminal_unit::exact_two_field_record_projection(
        program,
        source_parameter.type_reference,
        *moved_field,
        target_parameter.type_reference,
    )?;
    Some(CheckedStructuralControlProjectedEdgeCleanupPlan {
        machine: machine.symbol,
        state: state.symbol,
        statement_ordinal: 0,
        target_state: target.symbol,
        transfer: CheckedStructuralControlProjectedTransferPlan {
            source_parameter_position: 0,
            path: vec![CheckedUnitStructuralPathSegment::Field(
                moved_field_identity,
            )],
            type_identity: moved_type_identity,
            target_parameter_position: 0,
        },
        residual_affine_discards: vec![CheckedUnitPartialAffineDiscardPlan {
            source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: 0,
            },
            path: vec![CheckedUnitStructuralPathSegment::Field(
                residual_field_identity,
            )],
            type_identity: residual_type_identity,
        }],
    })
}

fn build_state_plan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
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
            let facts::PlaceRoot::Symbol(root) = event.root else {
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
    machine: symbols::SymbolHandle,
    state: &typed_trees::state::State,
) -> Option<Vec<(symbols::SymbolHandle, u32)>> {
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
                || entry_claim_roots.contains(&facts::PlaceRoot::Symbol(parameter.symbol))
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
        let facts::PlaceRoot::Symbol(root) = event.root else {
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

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
    let moves = super::discover_state_move_events(
        program,
        &facts.borrow,
        &facts.operators,
        machine,
        state,
        &mut segments,
    );
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
    let transferred_results = wholly_transferred_result_locals(program, facts, machine, state)?;
    let discard_parameters = checked_whole_affine_discard_parameters_excluding_results(
        program,
        facts,
        machine.symbol,
        state,
        &transferred_results,
    )?;

    let statements = program.statement_table.statements(state.statement_nodes);
    let has_structural_control = statements.iter().any(|statement| {
        let StatementNode::Transition(transition) = statement else {
            return false;
        };
        transition.exit == TransitionExit::Ordinary
            && matches!(
                program.statement_table.transition_target(transition.target),
                TransitionTargetNode::Named { path, .. }
                    if crate::checks::termination::named_transition_target_state_index(
                        program, machine, path.symbol,
                    ).is_some()
            )
    });
    if !has_structural_control {
        return None;
    }

    let mut segments = facts.flow.ownership.segments.clone();
    let moves = super::discover_state_move_events(
        program,
        &facts.borrow,
        &facts.operators,
        machine,
        state,
        &mut segments,
    );
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
        let Some(target_index) = crate::checks::termination::named_transition_target_state_index(
            program,
            machine,
            path.symbol,
        ) else {
            continue;
        };
        let target = &program.machine_states(machine)[target_index];

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
            // Entry back-edges name the machine in source; consumers join the
            // cleanup to the normalized state, while move events above retain
            // their authored call target identity.
            target_state: target.symbol,
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
    checked_whole_affine_discard_parameters_excluding_results(program, facts, machine, state, &[])
}

fn checked_whole_affine_discard_parameters_excluding_results(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: &typed_trees::state::State,
    transferred_results: &[symbols::SymbolHandle],
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
        if matches!(event.root, facts::PlaceRoot::Symbol(symbol) if transferred_results.contains(&symbol))
        {
            continue;
        }
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

/// Separate fresh results only after every actual outgoing edge transfers the
/// whole value. The retained cleanup row continues to describe parameters; an
/// unhandled local, return, continuation, or duplicate transfer rejects instead.
fn wholly_transferred_result_locals(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<Vec<symbols::SymbolHandle>> {
    let parameters = program.state_parameters(state);
    let statements = program.statement_table.statements(state.statement_nodes);
    let permissions = &facts.flow.ownership;
    let mut results = Vec::new();
    for (_, drop) in permissions.permissions.iter().filter(|(_, event)| {
        event.machine_symbol == machine.symbol
            && event.state_symbol == state.symbol
            && event.source == PermissionEventSource::StateExit
            && event.kind == PermissionEventKind::AffineDrop
    }) {
        let facts::PlaceRoot::Symbol(symbol) = drop.root else {
            return None;
        };
        if parameters
            .iter()
            .any(|parameter| parameter.symbol == symbol)
        {
            continue;
        }
        let PermissionProvenance::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Statement { statement_index },
        } = drop.provenance
        else {
            return None;
        };
        if machine_symbol != machine.symbol
            || state_symbol != state.symbol
            || drop.multiplicity != Multiplicity::Affine
            || drop.access != PermissionAccess::Owned
            || drop.claim_identity != PermissionClaimIdentity::Unknown
            || drop.obligation_live
            || !drop.segments.is_empty()
            || results.contains(&symbol)
        {
            return None;
        }
        let StatementNode::LocalData(local) = statements.get(statement_index)? else {
            return None;
        };
        if local.symbol != symbol
            || local.is_mutable
            || program.type_multiplicity(local.type_reference) != Multiplicity::Affine
            || !matches!(
                program.expression_table.expression(local.initial_value),
                typed_trees::expression::ExpressionNode::Call(_)
            )
            || !validation::has_plain_owned_contents_with_numeric_constraints(
                program,
                local.type_reference,
            )
        {
            return None;
        }
        let exact = |event: &checked_trees::FlowPermissionEventFact| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.root == drop.root
                && event.multiplicity == drop.multiplicity
                && event.access == drop.access
                && event.claim_identity == drop.claim_identity
                && event.provenance == drop.provenance
                && !event.obligation_live
                && event.segments.is_empty()
        };
        let mut establishments = permissions.permissions.iter().filter(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.root == drop.root
                && event.kind == PermissionEventKind::Establish
        });
        let (_, establish) = establishments.next()?;
        if establishments.next().is_some()
            || !exact(establish)
            || establish.source != (PermissionEventSource::Statement { statement_index })
        {
            return None;
        }
        if statements[..=statement_index]
            .iter()
            .any(|statement| matches!(statement, StatementNode::Transition(_)))
        {
            return None;
        }
        let mut edge_count = 0;
        for (ordinal, statement) in statements.iter().enumerate().skip(statement_index + 1) {
            let StatementNode::Transition(transition) = statement else {
                if matches!(statement, StatementNode::Expression(expression) if !matches!(program.expression_table.expression(*expression), typed_trees::expression::ExpressionNode::Call(_)))
                {
                    return None;
                }
                continue;
            };
            if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
                return None;
            }
            let TransitionTargetNode::Named {
                path, arguments, ..
            } = program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            let target_index = crate::checks::termination::named_transition_target_state_index(
                program,
                machine,
                path.symbol,
            )?;
            let target = &program.machine_states(machine)[target_index];
            let arguments = program.statement_table.expression_handles(*arguments);
            let target_parameters = program
                .state_parameters(target)
                .iter()
                .filter(|parameter| !parameter.is_self);
            if target_parameters.clone().count() != arguments.len() {
                return None;
            }
            let mut matching = arguments
                .iter()
                .zip(target_parameters)
                .filter(|(argument, _)| {
                    let typed_trees::expression::ExpressionNode::Name(name) =
                        program.expression_table.expression(**argument)
                    else {
                        return false;
                    };
                    name.symbol == symbol
                        && name.head_symbol == symbol
                        && program
                            .expression_table
                            .name_path_members(name.members)
                            .len()
                            == 1
                });
            let (_, parameter) = matching.next()?;
            if matching.next().is_some()
                || program.normalized_type_identity(parameter.type_reference)
                    != program.normalized_type_identity(local.type_reference)
            {
                return None;
            }
            let mut transfers = permissions.permissions.iter().filter(|(_, event)| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.root == drop.root
                    && event.kind == PermissionEventKind::Transfer
                    && matches!(event.source,
                        PermissionEventSource::Call { statement_index, .. }
                        | PermissionEventSource::Statement { statement_index }
                        if statement_index == ordinal)
            });
            let (_, transfer) = transfers.next()?;
            if transfers.next().is_some()
                || !exact(transfer)
                || transfer.source
                    != (PermissionEventSource::Call {
                        statement_index: ordinal,
                        call_ordinal: 0,
                        target_symbol: path.symbol,
                    })
            {
                return None;
            }
            edge_count += 1;
        }
        let retained_transfers = permissions
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.root == drop.root
                    && event.kind == PermissionEventKind::Transfer
            })
            .count();
        if edge_count == 0 || retained_transfers != edge_count {
            return None;
        }
        results.push(symbol);
    }
    Some(results)
}

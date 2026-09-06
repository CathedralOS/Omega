//! Structural control and boundary-machine construction.

use super::*;
use checked_trees::{
    CheckedStructuralRankedArgumentPlan, CheckedStructuralRankedGuardPlan,
    CheckedStructuralRankedSccEdgePlan, CheckedStructuralRankedSccPlan,
};

mod call_occurrences;

pub(crate) fn build_checked_structural_unit_control_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedStructuralUnitControlPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_structural_unit_control_machine(program, facts, &mut shapes, machine)
        })
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|machine| {
            std::iter::once(machine.attachment_type_identity.as_str()).chain(
                machine
                    .states
                    .iter()
                    .flat_map(|state| &state.structural_parameters)
                    .map(|parameter| parameter.type_identity.as_str()),
            )
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedStructuralUnitControlPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
    }
}

/// Bind one closed scalar return to an exact affine structural entry frontier.
/// This is deliberately separate from the primitive scalar graph: structural
/// parameters are custody, not fake scalar arguments.
pub(super) fn build_structural_unit_control_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedStructuralUnitControlMachinePlan> {
    let states = program.machine_states(machine);
    if states.len() < 2 {
        return None;
    }
    let proven_ranked_sccs =
        crate::checks::termination::proven_nat_countdown_sccs(program, machine)?;
    let proven_ranked_scc = match proven_ranked_sccs.as_slice() {
        [] => None,
        [component] => Some(component),
        _ => return None,
    };
    let binders = machine_binders(program, machine);
    let mut signatures = Vec::with_capacity(states.len());
    let mut attachment_type_identity = None;
    for state in states {
        if !is_unit(program, state.return_type)
            || !program.state_contracts(state).is_empty()
            || facts.flow.ownership.permissions.iter().any(|(_, event)| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.source == PermissionEventSource::StateEntry
                    && event.kind == PermissionEventKind::Establish
                    && event.access == PermissionAccess::Owned
            })
        {
            return None;
        }
        let flow = state_flow(facts, machine.symbol, state.symbol)?;
        if !facts
            .service_reaches
            .rows
            .services(flow.service_reach.direct)
            .is_empty()
            || !facts
                .service_reaches
                .rows
                .services(flow.service_reach.transitive)
                .is_empty()
        {
            return None;
        }
        let (attachment, structural_parameters, scalar_parameters) =
            structural_scalar_signature(program, shapes, machine, state, &binders, true)?;
        let parameters = structural_parameters;
        if parameters.is_empty()
            || parameters.iter().any(|parameter| {
                if parameter.is_self {
                    parameter.access != CheckedStructuralAccess::MutableBorrow
                        || !parameter.qualifications.is_empty()
                } else {
                    parameter.multiplicity != Multiplicity::Affine
                        || !parameter.qualifications.is_empty()
                }
            })
            || parameters.len() + scalar_parameters.len() != program.state_parameters(state).len()
        {
            return None;
        }
        if attachment_type_identity
            .as_ref()
            .is_some_and(|identity| identity != &attachment)
        {
            return None;
        }
        attachment_type_identity = Some(attachment);
        signatures.push((parameters, scalar_parameters));
    }

    let mut checked_states = Vec::with_capacity(states.len());
    for (state_index, state) in states.iter().enumerate() {
        let (source_parameters, source_scalar_parameters) = &signatures[state_index];
        let statements = program.statement_table.statements(state.statement_nodes);
        let terminator = match statements {
            [] => CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                trivial_affine_discard_parameter_positions:
                    checked_no_code_affine_discard_positions(program, facts, machine.symbol, state)?,
            },
            [StatementNode::Transition(transition)]
                if transition.exit == TransitionExit::Ordinary
                    && transition.guard == TransitionGuardNode::Always
                    && !transition.continuation.is_valid() =>
            {
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
                let (target_parameters, target_scalar_parameters) = &signatures[target_index];
                let arguments = program.statement_table.expression_handles(*arguments);
                if arguments.len()
                    != target_parameters
                        .iter()
                        .filter(|parameter| !parameter.is_self)
                        .count()
                        + target_scalar_parameters.len()
                {
                    return None;
                }
                let mut transferred_sources = BTreeSet::new();
                let transfers = target_parameters
                    .iter()
                    .enumerate()
                    .map(|(target_parameter_index, target)| {
                        if target.is_self {
                            let source_index =
                                source_parameters.iter().position(|source| source.is_self)?;
                            let source = &source_parameters[source_index];
                            if source != target || !transferred_sources.insert(source_index) {
                                return None;
                            }
                            return Some(CheckedStructuralControlTransferPlan {
                                source_parameter_index: u32::try_from(source_index).ok()?,
                                target_parameter_index: u32::try_from(target_parameter_index)
                                    .ok()?,
                            });
                        }
                        let argument_index = program
                            .state_parameters(&states[target_index])
                            .iter()
                            .take(target.position as usize)
                            .filter(|parameter| !parameter.is_self)
                            .count();
                        let argument = arguments.get(argument_index)?;
                        let place = crate::flow::canonical_place_from_expression_in_state(
                            program,
                            state.symbol,
                            0,
                            *argument,
                        )?;
                        let facts::PlaceRoot::Symbol(root) = place.root else {
                            return None;
                        };
                        if !place.segments.is_empty() {
                            return None;
                        }
                        let source_index = source_parameters.iter().position(|source| {
                            let source = program
                                .state_parameters(state)
                                .get(source.position as usize);
                            source.is_some_and(|source| source.symbol == root)
                        })?;
                        let source = &source_parameters[source_index];
                        if source.type_identity != target.type_identity
                            || source.multiplicity != target.multiplicity
                            || !transferred_sources.insert(source_index)
                        {
                            return None;
                        }
                        Some(CheckedStructuralControlTransferPlan {
                            source_parameter_index: u32::try_from(source_index).ok()?,
                            target_parameter_index: u32::try_from(target_parameter_index).ok()?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let scalar_arguments =
                    target_scalar_parameters
                        .iter()
                        .enumerate()
                        .map(|(target_index, target)| {
                            let argument_ordinal = target.source_position;
                            let ranked_edge = proven_ranked_scc.and_then(|component| {
                                component.covered_cyclic_edges.iter().find(|edge| {
                                    edge.source_state == state.symbol
                                        && edge.target_state == states[target_index].symbol
                                        && edge.statement_ordinal == 0
                                        && edge.target_rank_parameter_position == argument_ordinal
                                })
                            });
                            let expression = facts.values.scalar_expressions.expression_at(
                                state.symbol,
                                0,
                                CheckedScalarExpressionRole::TransitionArgument {
                                    argument_ordinal,
                                },
                            );
                            let source_index = match expression {
                                Some(CheckedScalarExpression::Boolean(expression))
                                    if target.primitive_type == PrimitiveType::Bool =>
                                {
                                    let checked_trees::CheckedBooleanExpression::Parameter {
                                        position,
                                    } = expression.as_ref()
                                    else {
                                        return None;
                                    };
                                    *position
                                }
                                Some(CheckedScalarExpression::Parameter {
                                    position,
                                    primitive_type,
                                }) if *primitive_type == target.primitive_type => *position,
                                _ if ranked_edge.is_some()
                                    && proven_ranked_scc.is_some_and(|component| {
                                        component.rank_primitive_type == target.primitive_type
                                    }) =>
                                {
                                    source_scalar_parameters.iter().position(|source| {
                                        ranked_edge.is_some_and(|edge| {
                                            source.source_position
                                                == edge.source_rank_parameter_position
                                        })
                                    })?
                                }
                                _ => return None,
                            };
                            if source_scalar_parameters
                                .get(source_index)
                                .is_none_or(|source| source.primitive_type != target.primitive_type)
                            {
                                return None;
                            }
                            Some(CheckedStructuralScalarArgumentPlan {
                                argument_ordinal,
                                source_scalar_parameter_index: u32::try_from(source_index).ok()?,
                                target_scalar_parameter_index: u32::try_from(target_index).ok()?,
                                primitive_type: target.primitive_type,
                            })
                        })
                        .collect::<Option<Vec<_>>>()?;
                let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
                    machine.symbol,
                    state.symbol,
                    0,
                )?;
                if cleanup.target_state != states[target_index].symbol {
                    return None;
                }
                let cleanup_sources = cleanup
                    .trivial_affine_discard_parameter_positions
                    .iter()
                    .map(|position| {
                        source_parameters
                            .iter()
                            .position(|parameter| parameter.position == *position)
                    })
                    .collect::<Option<BTreeSet<_>>>()?;
                if !transferred_sources.is_disjoint(&cleanup_sources)
                    || transferred_sources
                        .union(&cleanup_sources)
                        .copied()
                        .collect::<BTreeSet<_>>()
                        != (0..source_parameters.len()).collect::<BTreeSet<_>>()
                {
                    return None;
                }
                CheckedStructuralUnitControlTerminatorPlan::Jump {
                    statement_ordinal: 0,
                    target_state: path.symbol,
                    transfers,
                    scalar_arguments,
                    trivial_affine_discard_parameter_positions: cleanup
                        .trivial_affine_discard_parameter_positions
                        .clone(),
                }
            }
            [
                StatementNode::Transition(when_true),
                StatementNode::Transition(when_false),
            ] if when_true.exit == TransitionExit::Ordinary
                && matches!(when_true.guard, TransitionGuardNode::When(_))
                && when_false.exit == TransitionExit::Ordinary
                && when_false.guard == TransitionGuardNode::Always
                && !when_true.continuation.is_valid()
                && !when_false.continuation.is_valid() =>
            {
                let guard_expression = facts.values.scalar_expressions.expression_at(
                    state.symbol,
                    0,
                    CheckedScalarExpressionRole::Guard,
                );
                let ranked_guard = proven_ranked_scc.and_then(|component| {
                    component.covered_cyclic_edges.iter().find(|edge| {
                        edge.source_state == state.symbol && edge.statement_ordinal == 0
                    })
                });
                let guard_scalar_parameter_index = match guard_expression {
                    Some(CheckedScalarExpression::Boolean(expression))
                        if matches!(
                            expression.as_ref(),
                            checked_trees::CheckedBooleanExpression::Parameter { .. }
                        ) =>
                    {
                        let checked_trees::CheckedBooleanExpression::Parameter { position } =
                            expression.as_ref()
                        else {
                            unreachable!()
                        };
                        let parameter = source_scalar_parameters.get(*position)?;
                        (parameter.primitive_type == PrimitiveType::Bool)
                            .then(|| u32::try_from(*position).ok())??
                    }
                    _ if ranked_guard.is_some() => {
                        let edge = ranked_guard?;
                        let index = source_scalar_parameters.iter().position(|parameter| {
                            parameter.source_position == edge.source_rank_parameter_position
                        })?;
                        let parameter = source_scalar_parameters.get(index)?;
                        (proven_ranked_scc?.rank_primitive_type == parameter.primitive_type)
                            .then(|| u32::try_from(index).ok())??
                    }
                    _ => return None,
                };
                let build_successor =
                    |statement_ordinal: u32,
                     transition: &typed_trees::statement::TableTransition|
                     -> Option<CheckedStructuralControlSuccessorPlan> {
                        let TransitionTargetNode::Named {
                            path, arguments, ..
                        } = program.statement_table.transition_target(transition.target)
                        else {
                            return None;
                        };
                        let target_index =
                            crate::checks::termination::named_transition_target_state_index(
                                program,
                                machine,
                                path.symbol,
                            )?;
                        let (target_parameters, target_scalar_parameters) =
                            &signatures[target_index];
                        let arguments = program.statement_table.expression_handles(*arguments);
                        if arguments.len()
                            != target_parameters
                                .iter()
                                .filter(|parameter| !parameter.is_self)
                                .count()
                                + target_scalar_parameters.len()
                        {
                            return None;
                        }
                        let mut transferred_sources = BTreeSet::new();
                        let transfers = target_parameters
                            .iter()
                            .enumerate()
                            .map(|(target_parameter_index, target)| {
                                if target.is_self {
                                    let source_index = source_parameters
                                        .iter()
                                        .position(|source| source.is_self)?;
                                    let source = &source_parameters[source_index];
                                    if source != target || !transferred_sources.insert(source_index)
                                    {
                                        return None;
                                    }
                                    return Some(CheckedStructuralControlTransferPlan {
                                        source_parameter_index: u32::try_from(source_index).ok()?,
                                        target_parameter_index: u32::try_from(
                                            target_parameter_index,
                                        )
                                        .ok()?,
                                    });
                                }
                                let argument_index = program
                                    .state_parameters(&states[target_index])
                                    .iter()
                                    .take(target.position as usize)
                                    .filter(|parameter| !parameter.is_self)
                                    .count();
                                let argument = arguments.get(argument_index)?;
                                let place = crate::flow::canonical_place_from_expression_in_state(
                                    program,
                                    state.symbol,
                                    usize::try_from(statement_ordinal).ok()?,
                                    *argument,
                                )?;
                                let facts::PlaceRoot::Symbol(root) = place.root else {
                                    return None;
                                };
                                if !place.segments.is_empty() {
                                    return None;
                                }
                                let source_index = source_parameters.iter().position(|source| {
                                    program
                                        .state_parameters(state)
                                        .get(source.position as usize)
                                        .is_some_and(|parameter| parameter.symbol == root)
                                })?;
                                let source = &source_parameters[source_index];
                                if source.type_identity != target.type_identity
                                    || source.multiplicity != target.multiplicity
                                    || !transferred_sources.insert(source_index)
                                {
                                    return None;
                                }
                                Some(CheckedStructuralControlTransferPlan {
                                    source_parameter_index: u32::try_from(source_index).ok()?,
                                    target_parameter_index: u32::try_from(target_parameter_index)
                                        .ok()?,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?;
                        let scalar_arguments =
                            target_scalar_parameters
                                .iter()
                                .enumerate()
                                .map(|(target_index, target)| {
                                    let argument_ordinal = target.source_position;
                                    let ranked_edge = proven_ranked_scc.and_then(|component| {
                                        component.covered_cyclic_edges.iter().find(|edge| {
                                            edge.source_state == state.symbol
                                                && edge.target_state == states[target_index].symbol
                                                && edge.statement_ordinal == statement_ordinal
                                                && edge.target_rank_parameter_position
                                                    == argument_ordinal
                                        })
                                    });
                                    let expression = facts.values.scalar_expressions.expression_at(
                                        state.symbol,
                                        statement_ordinal,
                                        CheckedScalarExpressionRole::TransitionArgument {
                                            argument_ordinal,
                                        },
                                    );
                                    let source_index = match expression {
                                        Some(CheckedScalarExpression::Boolean(expression))
                                            if target.primitive_type == PrimitiveType::Bool =>
                                        {
                                            let checked_trees::CheckedBooleanExpression::Parameter {
                                            position,
                                        } = expression.as_ref()
                                        else {
                                            return None;
                                        };
                                            *position
                                        }
                                        Some(CheckedScalarExpression::Parameter {
                                            position,
                                            primitive_type,
                                        }) if *primitive_type == target.primitive_type => *position,
                                        _ if ranked_edge.is_some()
                                            && proven_ranked_scc.is_some_and(|component| {
                                                component.rank_primitive_type
                                                    == target.primitive_type
                                            }) =>
                                        {
                                            source_scalar_parameters.iter().position(|source| {
                                                ranked_edge.is_some_and(|edge| {
                                                    source.source_position
                                                        == edge.source_rank_parameter_position
                                                })
                                            })?
                                        }
                                        _ => return None,
                                    };
                                    if source_scalar_parameters.get(source_index).is_none_or(
                                        |source| source.primitive_type != target.primitive_type,
                                    ) {
                                        return None;
                                    }
                                    Some(CheckedStructuralScalarArgumentPlan {
                                        argument_ordinal,
                                        source_scalar_parameter_index: u32::try_from(source_index)
                                            .ok()?,
                                        target_scalar_parameter_index: u32::try_from(target_index)
                                            .ok()?,
                                        primitive_type: target.primitive_type,
                                    })
                                })
                                .collect::<Option<Vec<_>>>()?;
                        let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
                            machine.symbol,
                            state.symbol,
                            statement_ordinal,
                        )?;
                        if cleanup.target_state != states[target_index].symbol {
                            return None;
                        }
                        let cleanup_sources = cleanup
                            .trivial_affine_discard_parameter_positions
                            .iter()
                            .map(|position| {
                                source_parameters
                                    .iter()
                                    .position(|parameter| parameter.position == *position)
                            })
                            .collect::<Option<BTreeSet<_>>>()?;
                        if !transferred_sources.is_disjoint(&cleanup_sources)
                            || transferred_sources
                                .union(&cleanup_sources)
                                .copied()
                                .collect::<BTreeSet<_>>()
                                != (0..source_parameters.len()).collect::<BTreeSet<_>>()
                        {
                            return None;
                        }
                        Some(CheckedStructuralControlSuccessorPlan {
                            statement_ordinal,
                            target_state: path.symbol,
                            transfers,
                            scalar_arguments,
                            trivial_affine_discard_parameter_positions: cleanup
                                .trivial_affine_discard_parameter_positions
                                .clone(),
                        })
                    };
                let when_true = build_successor(0, when_true)?;
                let when_false = build_successor(1, when_false)?;
                if when_true.target_state == when_false.target_state {
                    return None;
                }
                CheckedStructuralUnitControlTerminatorPlan::Conditional {
                    guard_scalar_parameter_index,
                    when_true,
                    when_false,
                }
            }
            _ => return None,
        };
        checked_states.push(CheckedStructuralUnitControlStatePlan {
            state: state.symbol,
            structural_parameters: source_parameters.clone(),
            scalar_parameters: source_scalar_parameters.clone(),
            terminator,
        });
    }
    if checked_states
        .iter()
        .filter(|state| {
            matches!(
                state.terminator,
                CheckedStructuralUnitControlTerminatorPlan::Conditional { .. }
            )
        })
        .count()
        > 2
    {
        return None;
    }
    let mut predecessor_counts = vec![0_usize; checked_states.len()];
    for state in &checked_states {
        let targets = match &state.terminator {
            CheckedStructuralUnitControlTerminatorPlan::ReturnUnit { .. } => Vec::new(),
            CheckedStructuralUnitControlTerminatorPlan::Jump { target_state, .. } => {
                vec![*target_state]
            }
            CheckedStructuralUnitControlTerminatorPlan::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target_state, when_false.target_state],
        };
        for target in targets {
            let target_index = checked_states
                .iter()
                .position(|candidate| candidate.state == target)?;
            let count = predecessor_counts.get_mut(target_index)?;
            *count += 1;
            if *count > 2 {
                return None;
            }
        }
    }
    if (predecessor_counts[0] != 0
        && proven_ranked_scc
            .is_none_or(|component| component.header_state != checked_states[0].state))
        || predecessor_counts
            .iter()
            .filter(|count| **count == 2)
            .count()
            > 1
    {
        return None;
    }
    let ranked_scc = if let Some(component) = proven_ranked_scc {
        let header = checked_states
            .iter()
            .find(|state| state.state == component.header_state)?;
        let rank_scalar_parameter_index =
            header.scalar_parameters.iter().position(|parameter| {
                parameter.source_position == component.header_rank_parameter_position
                    && parameter.primitive_type == component.rank_primitive_type
            })?;
        let covered_cyclic_edges = component
            .covered_cyclic_edges
            .iter()
            .map(|edge| {
                let source = checked_states
                    .iter()
                    .find(|state| state.state == edge.source_state)?;
                let target = checked_states
                    .iter()
                    .find(|state| state.state == edge.target_state)?;
                let source_index = source.scalar_parameters.iter().position(|parameter| {
                    parameter.source_position == edge.source_rank_parameter_position
                        && parameter.primitive_type == component.rank_primitive_type
                })?;
                let target_index = target.scalar_parameters.iter().position(|parameter| {
                    parameter.source_position == edge.target_rank_parameter_position
                        && parameter.primitive_type == component.rank_primitive_type
                })?;
                Some(CheckedStructuralRankedSccEdgePlan {
                    source_state: edge.source_state,
                    target_state: edge.target_state,
                    statement_ordinal: edge.statement_ordinal,
                    guard: CheckedStructuralRankedGuardPlan::UnsignedParameterPositive {
                        scalar_parameter_index: u32::try_from(source_index).ok()?,
                        primitive_type: component.rank_primitive_type,
                    },
                    successor_argument:
                        CheckedStructuralRankedArgumentPlan::UnsignedParameterMinusOne {
                            argument_ordinal: edge.target_rank_parameter_position,
                            source_scalar_parameter_index: u32::try_from(source_index).ok()?,
                            target_scalar_parameter_index: u32::try_from(target_index).ok()?,
                            primitive_type: component.rank_primitive_type,
                        },
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(CheckedStructuralRankedSccPlan {
            header_state: component.header_state,
            rank_scalar_parameter_index: u32::try_from(rank_scalar_parameter_index).ok()?,
            rank_primitive_type: component.rank_primitive_type,
            rank_lower_bound: component.rank_lower_bound,
            rank_upper_bound: component.rank_upper_bound,
            covered_cyclic_edges,
        })
    } else {
        None
    };
    Some(CheckedStructuralUnitControlMachinePlan {
        machine: machine.symbol,
        attachment_type_identity: attachment_type_identity?,
        states: checked_states,
        ranked_scc,
    })
}

pub(super) fn build_boundary_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedBoundaryMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let binders = machine_binders(program, machine);
    let result = boundary_result_plan(program, shapes, state.return_type, &binders)?;
    if !program
        .statement_table
        .statements(state.statement_nodes)
        .is_empty()
    {
        return None;
    }
    let (attachment_type_identity, structural_parameters, scalar_parameters) =
        structural_scalar_signature(program, shapes, machine, state, &binders, false)?;
    let domain_requirements = boundary_domain_requirements(
        program,
        facts,
        shapes,
        machine,
        state,
        &structural_parameters,
        &binders,
    )?;
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;

    Some(CheckedBoundaryMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        contract_owner: machine.symbol,
        attachment_type_identity: Some(attachment_type_identity),
        structural_parameters,
        scalar_parameters,
        result,
        domain_requirements,
        contract_report_fingerprint: contract.report_fingerprint,
        contract_commitment: contract.commitment,
        contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
        service_reach: state_flow.service_reach,
    })
}

/// Project the narrow static boundary-trait surface used by checked-adapter
/// dispatch. A trait requirement is not an attached machine and therefore
/// contributes no provider value or structural attachment.
pub(super) fn build_static_boundary_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
) -> Vec<CheckedBoundaryMachinePlan> {
    let mut plans = Vec::new();
    for definition in program.traits().iter().filter(|definition| {
        definition.is_boundary && program.trait_type_parameters(definition).is_empty()
    }) {
        for signature in program.trait_machine_signatures(definition) {
            let type_parameters = program.state_signature_type_parameters(signature);
            let direct_callback_telescope = !signature.native_callback_parameters.is_empty()
                && type_parameters.len() == signature.native_callback_parameters.len()
                && type_parameters
                    .iter()
                    .zip(&signature.native_callback_parameters)
                    .all(|(parameter, callback)| {
                        parameter.name == callback.binder
                            && matches!(
                                parameter.kind,
                                typed_trees::data::TypeParameterKind::Machine {
                                    contract:
                                        typed_trees::data::MachineParameterContract::Nominal { .. }
                                }
                            )
                    });
            if (!type_parameters.is_empty() && !direct_callback_telescope)
                || !signature_contracts_are_exact_parameter_qualifications(program, signature)
                || signature.suspends
                || signature.blocks
            {
                continue;
            }
            let Some(result) = boundary_result_plan(program, shapes, signature.return_type, &[])
            else {
                continue;
            };
            let mut structural_parameters = Vec::new();
            let mut scalar_parameters = Vec::new();
            let mut supported = true;
            let mut abi_position = 0_usize;
            for parameter in program.state_signature_parameters(signature) {
                // A boundary-trait receiver selects the provider occurrence;
                // it is not an outbound ABI argument. Its progress premise is
                // closed by installation rather than materialized here.
                if parameter.is_self {
                    continue;
                }
                let Some(source_position) = u32::try_from(abi_position).ok() else {
                    supported = false;
                    break;
                };
                abi_position += 1;
                // `is_mutable` is set by a `mut` binding and by an exclusive
                // borrow alike. An exclusive borrow is already carried exactly
                // by the parameter's structural access below, so only an owned
                // mutable binding stays outside this requirement surface.
                if parameter.is_const
                    || (parameter.is_mutable && !is_reference(program, parameter.type_reference))
                {
                    supported = false;
                    break;
                }
                if let Some(primitive_type) =
                    program.primitive_type_reference(parameter.type_reference)
                {
                    scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                        source_position,
                        primitive_type,
                    });
                    continue;
                }
                let Some(type_identity) = shapes.add_type(parameter.type_reference, &[], &[])
                else {
                    supported = false;
                    break;
                };
                let Some(qualifications) =
                    parameter_qualifications(program, shapes, parameter.type_reference, &[])
                else {
                    supported = false;
                    break;
                };
                if is_reference(program, parameter.type_reference)
                    && byte_sequence_carrier(program, parameter.type_reference, &[])
                        != Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView)
                {
                    supported = false;
                    break;
                }
                let Some(access) =
                    structural_access_for_type_reference(program, parameter.type_reference)
                else {
                    supported = false;
                    break;
                };
                structural_parameters.push(CheckedUnitStructuralParameterPlan {
                    position: source_position,
                    is_self: false,
                    type_identity,
                    multiplicity: crate::checks::type_multiplicity(
                        program,
                        parameter.type_reference,
                    ),
                    access,
                    qualifications,
                    fused_service_erasure: None,
                });
            }
            if !supported {
                continue;
            }
            let Some(capsule) = facts
                .contract_plans
                .crash_capsule(definition.symbol, signature.symbol)
            else {
                continue;
            };
            let call_reaches = facts
                .flow
                .control
                .calls
                .iter()
                .map(|(_, call)| call)
                .filter(|call| {
                    call.target_symbol == signature.symbol
                        || exact_compiler_intrinsic_boundary_requirement(
                            program,
                            call.target_symbol,
                        )
                        .is_some_and(|(requirement, _)| requirement == signature.symbol)
                        || program
                            .machine_parameter_signature(call.target_symbol)
                            .is_some_and(|(_, requirement)| requirement.symbol == signature.symbol)
                })
                .map(|call| call.service_reach.transitive)
                .collect::<Vec<_>>();
            let [published_reach, rest @ ..] = call_reaches.as_slice() else {
                continue;
            };
            if rest.iter().any(|reach| reach != published_reach) {
                continue;
            }
            let service_reach = language_semantics::ServiceReachSummary {
                direct: *published_reach,
                transitive: *published_reach,
            };
            let domain_requirements = structural_parameters
                .iter()
                .enumerate()
                .flat_map(|(argument_index, parameter)| {
                    parameter.qualifications.iter().map(move |domain| {
                        CheckedUnitStructuralDomainRequirementPlan {
                            argument_index: u32::try_from(argument_index)
                                .expect("structural parameter count already fits source positions"),
                            domain: *domain,
                        }
                    })
                })
                .collect();
            plans.push(CheckedBoundaryMachinePlan {
                machine: signature.symbol,
                state: signature.symbol,
                contract_owner: definition.symbol,
                attachment_type_identity: None,
                structural_parameters,
                scalar_parameters,
                result,
                domain_requirements,
                contract_report_fingerprint: capsule.target_contract_report_fingerprint(),
                contract_commitment: capsule.target_contract_commitment(),
                contract_service_reach: language_semantics::ServiceReachPlan {
                    interface: language_semantics::ServiceReachInterface::PublishedCeiling(
                        *published_reach,
                    ),
                    checked_inferred: *published_reach,
                },
                service_reach,
            });
        }
    }
    plans.sort_by_key(|plan| (plan.machine.arena_index(), plan.machine.generation()));
    plans.dedup_by_key(|plan| plan.machine);
    plans
}

fn boundary_result_plan(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    type_reference: typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<CheckedBoundaryMachineResultPlan> {
    if is_unit(program, type_reference) {
        return Some(CheckedBoundaryMachineResultPlan::Unit);
    }
    if let Some(scalar) = program.primitive_type_reference(type_reference) {
        return Some(CheckedBoundaryMachineResultPlan::Scalar(scalar));
    }
    if is_reference(program, type_reference)
        || type_graph_requires_nominal_drop(program, type_reference)
    {
        return None;
    }
    Some(CheckedBoundaryMachineResultPlan::Structural {
        type_identity: shapes.add_type(type_reference, binders, &[])?,
        multiplicity: crate::checks::type_multiplicity(program, type_reference),
        qualifications: parameter_qualifications(program, shapes, type_reference, binders)?,
    })
}

pub(super) fn build_checked_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
) -> Option<CheckedUnitEffectMachinePlan> {
    build_checked_machine_with(
        program,
        facts,
        shapes,
        machine,
        selected_operator_applications,
        selected_ieee_float_fma_applications,
        false,
    )
    .or_else(|| {
        // ponytail: a borrowed `self` becomes structural parameter 0 only when
        // the ambient attachment cannot plan the body, so every machine that
        // planned before keeps its exact shape. Retain it unconditionally once
        // `validate_provider_attachment_specialization` (omega) admits a self
        // beside provider roots and the entry bridge passes the loan.
        let [state] = program.machine_states(machine) else {
            return None;
        };
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.is_self && is_reference(program, parameter.type_reference))
            .then(|| {
                build_checked_machine_with(
                    program,
                    facts,
                    shapes,
                    machine,
                    selected_operator_applications,
                    selected_ieee_float_fma_applications,
                    true,
                )
            })
            .flatten()
    })
}

fn build_checked_machine_with(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
    retain_reference_self: bool,
) -> Option<CheckedUnitEffectMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if !is_unit(program, state.return_type) {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let selected_scalar_result_local = selected_operator_scalar_result_local(
        program,
        machine,
        state,
        statements,
        selected_operator_applications,
    );
    let selected_structural_result_local = selected_operator_structural_result_local(
        program,
        shapes,
        machine,
        state,
        statements,
        selected_operator_applications,
    );
    let selected_structural_result_symbol = selected_structural_result_local
        .as_ref()
        .map(|(_, _, symbol)| *symbol);
    let binders = machine_binders(program, machine);
    let boundary_structural_result_local = selected_structural_result_local
        .is_none()
        .then(|| {
            checked_unit_boundary_structural_result_local(program, shapes, statements, &binders)
        })
        .flatten();
    let boundary_structural_result_symbol = boundary_structural_result_local
        .as_ref()
        .map(|(_, symbol)| *symbol);
    let carries_fused_service_parameter = program.state_parameters(state).iter().any(|parameter| {
        typed_trees::service::exact_bound_service_requirement(program, parameter.type_reference)
            .is_some()
    });
    let carries_scalar_parameter = program.state_parameters(state).iter().any(|parameter| {
        !parameter.is_self
            && program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
    });
    let (attachment_type_identity, mut structural_parameters, scalar_parameters) =
        if machine.attached_data.is_none() {
            if carries_fused_service_parameter {
                let (structural, scalar) =
                    free_fused_service_scalar_signature(program, shapes, state, &binders)?;
                (None, structural, scalar)
            } else if (selected_scalar_result_local.is_some()
                || selected_structural_result_local.is_some()
                || boundary_structural_result_local.is_some())
                && !carries_scalar_parameter
            {
                let structural =
                    free_selected_operator_structural_signature(program, shapes, state, &binders)?;
                (None, structural, Vec::new())
            } else {
                return None;
            }
        } else if carries_fused_service_parameter {
            let (attachment, structural, scalar) = fused_service_scalar_signature(
                program,
                shapes,
                machine,
                state,
                &binders,
                retain_reference_self,
            )?;
            (Some(attachment), structural, scalar)
        } else if carries_scalar_parameter {
            let (attachment, structural, scalar) = structural_scalar_signature(
                program,
                shapes,
                machine,
                state,
                &binders,
                retain_reference_self,
            )?;
            (Some(attachment), structural, scalar)
        } else {
            let (attachment, structural) = structural_signature(
                program,
                shapes,
                machine,
                state,
                &binders,
                retain_reference_self,
            )?;
            (Some(attachment), structural, Vec::new())
        };
    if !checked_state_contracts_supported(program, machine, state, &structural_parameters) {
        return None;
    }
    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let source_calls = facts.flow.control.calls.span_or_empty(state_flow.calls);
    let calls = call_occurrences::outer_calls(program, facts, machine.symbol, state, source_calls)?;
    let construction = build_affine_array_construction_prefix(
        program, facts, shapes, machine, state, &binders, statements,
    );
    let affine_scalar_record_local = construction
        .is_none()
        .then(|| {
            build_unit_affine_scalar_record_local(
                program, facts, shapes, machine, state, &binders, statements,
            )
        })
        .flatten();
    let selected_ieee_float_fma_result_locals = selected_ieee_float_fma_result_locals(
        program,
        machine,
        state,
        statements,
        selected_ieee_float_fma_applications,
    );
    if (selected_scalar_result_local.is_some()
        || selected_structural_result_local.is_some()
        || boundary_structural_result_local.is_some())
        && selected_ieee_float_fma_result_locals.is_some()
    {
        return None;
    }
    let scalar_result_local = (selected_scalar_result_local.is_none()
        && selected_structural_result_local.is_none()
        && boundary_structural_result_local.is_none()
        && selected_ieee_float_fma_result_locals.is_none())
    .then(|| checked_unit_scalar_result_local(program, statements))
    .flatten();
    let selected_write_only_scalar_result_local = selected_scalar_result_local
        .as_ref()
        .map(|(_, result)| result);
    let write_only_scalar_result_local = scalar_result_local
        .as_ref()
        .or(selected_write_only_scalar_result_local);
    let write_only_store = build_write_only_primitive_store(
        program,
        facts,
        shapes,
        machine,
        state,
        &structural_parameters,
        &scalar_parameters,
        statements,
        scalar_result_local.as_ref(),
        selected_write_only_scalar_result_local,
    );
    let structural_scalar_field_store = write_only_store
        .is_none()
        .then(|| {
            build_structural_scalar_field_store(
                program,
                facts,
                machine,
                state,
                &structural_parameters,
                &scalar_parameters,
                statements,
                scalar_result_local.as_ref(),
                selected_write_only_scalar_result_local,
            )
        })
        .flatten();
    let scalar_expression_locals =
        if selected_scalar_result_local.is_some() || scalar_result_local.is_some() {
            scalar_expression_local_suffix(program, facts, state, statements)?
        } else {
            super::scalar_locals::scalar_expression_local_prefix(program, facts, state, statements)
                .unwrap_or_default()
        };
    let scalar_result_local_count = selected_ieee_float_fma_result_locals.as_ref().map_or_else(
        || {
            usize::from(
                scalar_result_local.is_some()
                    || selected_scalar_result_local.is_some()
                    || selected_structural_result_local.is_some()
                    || boundary_structural_result_local.is_some(),
            ) + scalar_expression_locals.len()
        },
        Vec::len,
    );
    let has_scalar_result_local = scalar_result_local_count != 0;
    if has_scalar_result_local && (construction.is_some() || affine_scalar_record_local.is_some()) {
        return None;
    }
    if write_only_store.is_some()
        && has_scalar_result_local
        && (write_only_scalar_result_local.is_none() || !scalar_expression_locals.is_empty())
    {
        return None;
    }
    let restored_call_alias_prefix =
        reborrow_restored_call_alias_prefix(program, facts, machine, state, statements);
    let local_count = if has_scalar_result_local {
        scalar_result_local_count
    } else if affine_scalar_record_local.is_some() {
        1
    } else {
        construction.as_ref().map_or_else(
            || {
                restored_call_alias_prefix.unwrap_or_else(|| {
                    statements
                        .iter()
                        .take_while(|statement| matches!(statement, StatementNode::LocalData(_)))
                        .count()
                })
            },
            |(_, local_statement_count)| *local_statement_count,
        )
    };
    let call_statements = if construction.is_some() {
        &statements[statements.len()..]
    } else {
        &statements[local_count..]
    };
    if write_only_store.is_some() || structural_scalar_field_store.is_some() {
        if construction.is_some()
            || affine_scalar_record_local.is_some()
            || if scalar_result_local.is_some() {
                local_count != 1 || calls.len() != 1 || statements.len() != 2
            } else if selected_scalar_result_local.is_some() {
                local_count != 1 || !calls.is_empty() || statements.len() != 2
            } else {
                local_count != 0 || !calls.is_empty()
            }
        {
            return None;
        }
    } else {
        let expected_call_count = call_statements.len().checked_add(usize::from(
            scalar_result_local.is_some() || boundary_structural_result_local.is_some(),
        ))?;
        if calls.len() != expected_call_count
            || call_statements
                .iter()
                .any(|statement| !matches!(statement, StatementNode::Call(_)))
        {
            return None;
        }
        // Primitive structural places are introduced solely for the bounded
        // write-only store/call closure. One exact empty write-only sink must
        // remain available as the target of a projected forwarding call; other
        // primitive leaves with no store and no call would widen the roster.
        let carries_primitive = structural_parameters.iter().any(|parameter| {
            shapes
                .types
                .get(&parameter.type_identity)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
                    )
                })
        });
        let exact_write_only_primitive_sink = matches!(
            structural_parameters.as_slice(),
            [parameter]
                if parameter.multiplicity == Multiplicity::Unrestricted
                    && parameter.access == CheckedStructuralAccess::WriteOnlyBorrow
                    && parameter.qualifications.is_empty()
                    && shapes.types.get(&parameter.type_identity).is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
                        )
                    })
        );
        let source_parameters = program.state_parameters(state);
        let exact_shared_primitive_observer = statements.is_empty()
            && calls.is_empty()
            && entry_claims.is_empty()
            && matches!(structural_parameters.len(), 2 | 3)
            && structural_parameters.iter().all(|parameter| {
                parameter.multiplicity == Multiplicity::Unrestricted
                    && parameter.access == CheckedStructuralAccess::SharedBorrow
                    && parameter.qualifications.is_empty()
                    && parameter.type_identity == structural_parameters[0].type_identity
            })
            && source_parameters.len() == structural_parameters.len()
            && source_parameters
                .iter()
                .all(|parameter| !parameter.is_self && !parameter.is_const);
        if carries_primitive
            && calls.is_empty()
            && !exact_write_only_primitive_sink
            && !exact_shared_primitive_observer
        {
            return None;
        }
    }
    let local_rows = match (
        has_scalar_result_local,
        construction,
        restored_call_alias_prefix,
        affine_scalar_record_local.as_ref(),
    ) {
        (true, None, None, None) => Vec::new(),
        (false, Some((rows, _)), None, None) => rows,
        (false, None, Some(_), None) | (false, None, None, Some(_)) => Vec::new(),
        (false, None, None, None) => build_unit_trivial_affine_locals(
            program,
            facts,
            shapes,
            machine,
            state,
            &binders,
            &statements[..local_count],
        )?,
        _ => {
            unreachable!("scalar, affine, and restored-alias local lanes were separated above")
        }
    };
    let trivial_affine_locals = local_rows
        .iter()
        .map(|(plan, _)| plan.clone())
        .collect::<Vec<_>>();
    let mut admitted_local_symbols = local_rows
        .iter()
        .map(|(_, symbol)| *symbol)
        .collect::<Vec<_>>();
    admitted_local_symbols.extend(
        affine_scalar_record_local
            .as_ref()
            .map(|local| local.symbol),
    );
    admitted_local_symbols.extend(selected_structural_result_symbol);
    admitted_local_symbols.extend(boundary_structural_result_symbol);

    let mut operations = trivial_affine_locals
        .iter()
        .map(
            |local| CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: local
                    .construction
                    .as_ref()
                    .and_then(|element| u32::try_from(element.index).ok())
                    .and_then(|index| index.checked_add(1))
                    .unwrap_or(local.declaration_ordinal),
                declaration_ordinal: local.declaration_ordinal,
                type_identity: local.type_identity.clone(),
            },
        )
        .collect::<Vec<_>>();
    if let Some(local) = &affine_scalar_record_local {
        operations.push(
            CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal {
                statement_index: local.declaration_ordinal,
                declaration_ordinal: local.declaration_ordinal,
                type_identity: local.type_identity.clone(),
                field_identity: local.field_identity.clone(),
                value: local.value.clone(),
            },
        );
    }
    if scalar_result_local.is_none() && selected_scalar_result_local.is_none() {
        operations.extend(
            scalar_expression_locals
                .iter()
                .cloned()
                .map(
                    |(result, value)| CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                        result,
                        value,
                    },
                ),
        );
    }
    operations.reserve(calls.len() + 1);
    if let Some(store) = write_only_store {
        if let Some((application, result)) = selected_scalar_result_local {
            operations.push(build_selected_operator_scalar_call(
                program,
                facts,
                state,
                application,
                result,
            )?);
        } else if let Some(result) = scalar_result_local {
            let call = calls.first()?;
            if call.statement_index != usize::try_from(result.statement_index).ok()?
                || call.call_ordinal != 0
            {
                return None;
            }
            let call_operation = build_call_operation(
                program,
                facts,
                machine,
                state,
                &structural_parameters,
                &local_rows,
                affine_scalar_record_local.as_slice(),
                &entry_claims,
                call,
                false,
                Some(ExpectedCallValueResult::Scalar(result.primitive_type)),
            )?;
            operations.push(bind_scalar_call_result(
                facts,
                call_operation,
                result,
                false,
            )?);
        }
        operations.push(store);
    } else if let Some(store) = structural_scalar_field_store {
        if let Some((application, result)) = selected_scalar_result_local {
            operations.push(build_selected_operator_scalar_call(
                program,
                facts,
                state,
                application,
                result,
            )?);
        } else if let Some(result) = scalar_result_local {
            let call = calls.first()?;
            if call.statement_index != usize::try_from(result.statement_index).ok()?
                || call.call_ordinal != 0
            {
                return None;
            }
            let call_operation = build_call_operation(
                program,
                facts,
                machine,
                state,
                &structural_parameters,
                &local_rows,
                affine_scalar_record_local.as_slice(),
                &entry_claims,
                call,
                false,
                Some(ExpectedCallValueResult::Scalar(result.primitive_type)),
            )?;
            operations.push(bind_scalar_call_result(
                facts,
                call_operation,
                result,
                false,
            )?);
        }
        operations.push(CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(
            store,
        ));
    } else {
        let call_offset =
            if let Some((application, result)) = selected_scalar_result_local {
                let operation =
                    build_selected_operator_scalar_call(program, facts, state, application, result)
                        .or_else(|| {
                            build_selected_operator_structural_scalar_call(
                                program,
                                facts,
                                shapes,
                                machine,
                                state,
                                &mut structural_parameters,
                                &entry_claims,
                                application,
                                result,
                            )
                        })?;
                operations.push(operation);
                operations.extend(scalar_expression_locals.iter().cloned().map(
                    |(result, value)| CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                        result,
                        value,
                    },
                ));
                0
            } else if let Some((application, result, _)) = selected_structural_result_local {
                operations.push(build_selected_operator_structural_call(
                    program,
                    facts,
                    shapes,
                    machine,
                    state,
                    &mut structural_parameters,
                    &entry_claims,
                    application,
                    result,
                )?);
                0
            } else if let Some(locals) = selected_ieee_float_fma_result_locals {
                for (application, result) in locals {
                    operations.push(build_selected_ieee_float_fma(
                        program,
                        facts,
                        state,
                        application,
                        result,
                    )?);
                }
                0
            } else if let Some((result, _)) = boundary_structural_result_local {
                let call = calls.first()?;
                if call.statement_index != usize::try_from(result.statement_index).ok()?
                    || call.call_ordinal != 0
                {
                    return None;
                }
                let CheckedUnitEffectOperationPlan::BoundaryCall {
                    coordinate,
                    source_site,
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    service_reach,
                    scalar_arguments,
                    structural_arguments,
                    completion_receipts,
                } = build_call_operation(
                    program,
                    facts,
                    machine,
                    state,
                    &structural_parameters,
                    &local_rows,
                    affine_scalar_record_local.as_slice(),
                    &entry_claims,
                    call,
                    false,
                    Some(ExpectedCallValueResult::Structural(&result)),
                )?
                else {
                    return None;
                };
                let discard_result_on_return = result.multiplicity == Multiplicity::Affine;
                operations.push(CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    coordinate,
                    source_site,
                    result,
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    service_reach,
                    scalar_arguments,
                    structural_arguments,
                    completion_receipts,
                    discard_result_on_return,
                });
                1
            } else if let Some(result) = scalar_result_local {
                let call = calls.first()?;
                if call.statement_index != usize::try_from(result.statement_index).ok()?
                    || call.call_ordinal != 0
                {
                    return None;
                }
                let call_operation = build_call_operation(
                    program,
                    facts,
                    machine,
                    state,
                    &structural_parameters,
                    &local_rows,
                    affine_scalar_record_local.as_slice(),
                    &entry_claims,
                    call,
                    false,
                    Some(ExpectedCallValueResult::Scalar(result.primitive_type)),
                )?;
                operations.push(bind_scalar_call_result(
                    facts,
                    call_operation,
                    result,
                    true,
                )?);
                operations.extend(scalar_expression_locals.iter().cloned().map(
                    |(result, value)| CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                        result,
                        value,
                    },
                ));
                1
            } else {
                0
            };
        for (call_index, call) in calls[call_offset..].iter().enumerate() {
            let statement_index = local_count.checked_add(call_index)?;
            if call.statement_index != statement_index || call.call_ordinal != 0 {
                return None;
            }
            operations.push(build_call_operation(
                program,
                facts,
                machine,
                state,
                &structural_parameters,
                &local_rows,
                affine_scalar_record_local.as_slice(),
                &entry_claims,
                call,
                false,
                None,
            )?);
        }
    }
    let transferred_local_ordinals = operations
        .iter()
        .flat_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                structural_arguments,
                ..
            } => structural_arguments
                .iter()
                .filter_map(|argument| {
                    argument.source_local_declaration_ordinal().or_else(|| {
                        argument.source_affine_scalar_record_local_declaration_ordinal()
                    })
                })
                .collect::<Vec<_>>(),
            CheckedUnitEffectOperationPlan::PortWrite { .. }
            | CheckedUnitEffectOperationPlan::ScalarCall { .. }
            | CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
            | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd { .. }
            | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
            | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
            | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
            | CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal { .. }
            | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
            | CheckedUnitEffectOperationPlan::ReturnUnit { .. } => Vec::new(),
        })
        .collect::<BTreeSet<_>>();
    operations.push(CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index: u32::try_from(statements.len()).ok()?,
        trivial_affine_local_discard_ordinals: (0..trivial_affine_locals.len()
            + usize::from(affine_scalar_record_local.is_some()))
            .rev()
            .map(|ordinal| u32::try_from(ordinal).ok())
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .filter(|ordinal| !transferred_local_ordinals.contains(ordinal))
            .collect(),
        trivial_affine_discards: return_unit_affine_discards(
            program,
            facts,
            machine.symbol,
            state.symbol,
            &structural_parameters,
            program.state_parameters(state),
            &operations,
            &admitted_local_symbols,
        )?,
    });

    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let mut body_qualifications = facts
        .qualifications
        .for_machine(machine.symbol)
        .map(|fact| fact.body_committed.clone())
        .unwrap_or_default();
    body_qualifications.sort_by_key(|domain| domain.0);
    body_qualifications.dedup();

    let provider_attachment_requirements = match attachment_type_identity.as_deref() {
        Some(attachment) => checked_provider_attachment_requirements(
            program,
            shapes,
            machine,
            state,
            attachment,
            &structural_parameters,
            source_calls,
            &operations,
        )?,
        None => Vec::new(),
    };

    Some(CheckedUnitEffectMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        scalar_parameters,
        provider_attachment_requirements,
        trivial_affine_locals,
        entry_claims,
        body_qualifications,
        contract_report_fingerprint: contract.report_fingerprint,
        contract_commitment: contract.commitment,
        contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
        service_reach: state_flow.service_reach,
        operations,
    })
}

/// Recognize only the erased parent alias and the one-, two-, or three-child roster
/// named by the checked
/// post-reactivation certificate. These source bindings carry borrow
/// lifetimes, not independent Terminal runtime places; every other reference
/// local continues to reject through the ordinary affine-local path.
fn reborrow_restored_call_alias_prefix(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
) -> Option<usize> {
    let StatementNode::LocalData(parent_local) = statements.first()? else {
        return None;
    };
    let child_count = facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .filter_map(|(_, certificate)| {
            (certificate.machine_symbol == machine.symbol
                && certificate.state_symbol == state.symbol)
                .then_some(())?;
            facts
                .borrow
                .reborrow_disposition_events
                .is_valid(certificate.disposition)
                .then(|| {
                    facts
                        .borrow
                        .reborrow_disposition_events
                        .get(certificate.disposition)
                        .shared_cohort
                        .len()
                        .max(1)
                })
        })
        .find(|count| matches!(count, 1..=3))?;
    let child_locals = statements
        .get(1..=child_count)?
        .iter()
        .map(|statement| match statement {
            StatementNode::LocalData(local) if !local.is_mutable => Some(local),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if parent_local.is_mutable {
        return None;
    }
    let ExpressionNode::Borrow(parent_borrow) = program
        .expression_table
        .expression(parent_local.initial_value)
    else {
        return None;
    };
    let child_borrows = child_locals
        .iter()
        .map(
            |local| match program.expression_table.expression(local.initial_value) {
                ExpressionNode::Borrow(borrow) => Some(borrow),
                _ => None,
            },
        )
        .collect::<Option<Vec<_>>>()?;
    if parent_borrow.access != language_semantics::ReferenceAccess::Mutable {
        return None;
    }

    let parent_source = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        0,
        parent_borrow.target,
    )?;
    let child_sources = child_borrows
        .iter()
        .enumerate()
        .map(|(offset, borrow)| {
            crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                offset + 1,
                borrow.target,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let candidates = facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .filter(|(_, certificate)| {
            if certificate.machine_symbol != machine.symbol
                || certificate.state_symbol != state.symbol
                || certificate.carrier_place.root_symbol != parent_local.symbol
                || !certificate.carrier_place.segments.is_empty()
                || parent_source.root
                    != facts::PlaceRoot::Symbol(certificate.restored_place.root_symbol)
                || parent_source.segments != certificate.restored_place.segments
                || child_sources.iter().any(|source| {
                    source.root != facts::PlaceRoot::Symbol(parent_local.symbol)
                        || !source.segments.is_empty()
                })
                || !facts
                    .borrow
                    .reborrow_loan_resources
                    .is_valid(certificate.child_resource)
                || !facts.flow.control.calls.is_valid(certificate.call)
            {
                return false;
            }
            let child = facts
                .borrow
                .reborrow_loan_resources
                .get(certificate.child_resource);
            let call = facts.flow.control.calls.get(certificate.call);
            let disposition = facts
                .borrow
                .reborrow_disposition_events
                .get(certificate.disposition);
            let roster = if disposition.shared_cohort.is_empty() {
                vec![certificate.child_resource]
            } else {
                disposition.shared_cohort.clone()
            };
            roster.len() == child_count
                && child_locals
                    .iter()
                    .zip(&child_borrows)
                    .all(|(local, borrow)| {
                        roster.iter().any(|resource| {
                            if !facts.borrow.reborrow_loan_resources.is_valid(*resource) {
                                return false;
                            }
                            let member = facts.borrow.reborrow_loan_resources.get(*resource);
                            member.owner_symbol == local.symbol
                                && borrow.access
                                    == match member.access {
                                        checked_trees::BorrowAccessKind::Mutable => {
                                            language_semantics::ReferenceAccess::Mutable
                                        }
                                        checked_trees::BorrowAccessKind::WriteOnly => {
                                            language_semantics::ReferenceAccess::WriteOnly
                                        }
                                        checked_trees::BorrowAccessKind::Read => {
                                            language_semantics::ReferenceAccess::Shared
                                        }
                                    }
                        })
                    })
                && roster.contains(&certificate.child_resource)
                && child_locals
                    .iter()
                    .any(|local| local.symbol == child.owner_symbol)
                && call.statement_index == child_count + usize::from(child_count > 1) + 1
                && call.call_ordinal == 0
                && call.target_symbol == certificate.target_symbol
        })
        .count();
    (candidates == 1).then_some(child_count + 1)
}

fn checked_unit_scalar_result_local(
    program: &TypedTrees,
    statements: &[StatementNode],
) -> Option<CheckedUnitScalarResultBindingPlan> {
    let StatementNode::LocalData(local) = statements.first()? else {
        return None;
    };
    if local.is_mutable || !local.initial_value.is_valid() {
        return None;
    }
    let primitive_type = program.primitive_type_reference(local.type_reference)?;
    if !matches!(
        program.expression_table.expression(local.initial_value),
        ExpressionNode::Call(_)
    ) {
        return None;
    }
    Some(CheckedUnitScalarResultBindingPlan {
        statement_index: 0,
        binding_ordinal: 0,
        primitive_type,
    })
}

fn bind_scalar_call_result(
    facts: &CheckFacts,
    operation: CheckedUnitEffectOperationPlan,
    result: CheckedUnitScalarResultBindingPlan,
    allow_boundary: bool,
) -> Option<CheckedUnitEffectOperationPlan> {
    match operation {
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            source_site,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            scalar_arguments,
            structural_arguments,
            completion_receipts,
        } if allow_boundary => Some(CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            source_site,
            result,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            scalar_arguments,
            structural_arguments,
            completion_receipts,
        }),
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            scalar_arguments,
            structural_arguments,
            claim_transfers,
        } if structural_arguments.is_empty() && claim_transfers.is_empty() => {
            Some(CheckedUnitEffectOperationPlan::ScalarCall {
                coordinate,
                result,
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                target_contract_commitment: facts
                    .contract_plans
                    .for_machine(target_machine)?
                    .commitment,
                service_reach,
                scalar_arguments,
            })
        }
        _ => None,
    }
}

pub(super) fn checked_unit_boundary_structural_result_local(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    statements: &[StatementNode],
    binders: &[(SymbolHandle, String)],
) -> Option<(CheckedUnitStructuralResultBindingPlan, SymbolHandle)> {
    let StatementNode::LocalData(local) = statements.first()? else {
        return None;
    };
    if local.is_mutable
        || !local.initial_value.is_valid()
        || program
            .primitive_type_reference(local.type_reference)
            .is_some()
        || is_reference(program, local.type_reference)
        || crate::checks::type_multiplicity(program, local.type_reference) == Multiplicity::Linear
        || type_graph_requires_nominal_drop(program, local.type_reference)
        || !matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        )
        || !parameter_qualifications(program, shapes, local.type_reference, binders)?.is_empty()
    {
        return None;
    }
    Some((
        CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_type(local.type_reference, binders, &[])?,
            multiplicity: crate::checks::type_multiplicity(program, local.type_reference),
        },
        local.symbol,
    ))
}

fn build_write_only_primitive_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statements: &[StatementNode],
    scalar_result_local: Option<&CheckedUnitScalarResultBindingPlan>,
    selected_scalar_result_local: Option<&CheckedUnitScalarResultBindingPlan>,
) -> Option<CheckedUnitEffectOperationPlan> {
    let result_local = scalar_result_local.or(selected_scalar_result_local);
    let (statement_index, assignment) = match (result_local, statements) {
        (None, [StatementNode::Assignment(assignment)]) => (0, assignment),
        (
            Some(result),
            [
                StatementNode::LocalData(_),
                StatementNode::Assignment(assignment),
            ],
        ) if result.statement_index == 0 && result.binding_ordinal == 0 => (1, assignment),
        _ => return None,
    };
    let [destination] = structural_parameters else {
        return None;
    };
    if destination.is_self
        || destination.position != 0
        || destination.multiplicity != Multiplicity::Unrestricted
        || !matches!(
            destination.access,
            CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
        )
        || !destination.qualifications.is_empty()
    {
        return None;
    }
    let CheckedUnitStructuralTypeShape::PrimitiveScalar(destination_type) = shapes
        .types
        .get(&destination.type_identity)
        .map(|declaration| &declaration.shape)?
    else {
        return None;
    };
    let parameter = program
        .state_parameters(state)
        .get(usize::try_from(destination.position).ok()?)?;
    if program.state_parameters(state).len()
        != structural_parameters.len() + scalar_parameters.len()
    {
        return None;
    }
    if parameter.is_self || parameter.is_const || !parameter.is_mutable {
        return None;
    }
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let expected_access = match access {
        language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        language_semantics::ReferenceAccess::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
        language_semantics::ReferenceAccess::Shared => return None,
    };
    if destination.access != expected_access
        || program.primitive_type_reference(*referee) != Some(*destination_type)
    {
        return None;
    }
    let target = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        usize::try_from(statement_index).ok()?,
        assignment.target,
    )?;
    if target.root != facts::PlaceRoot::Symbol(parameter.symbol) || !target.segments.is_empty() {
        return None;
    }
    let frame = &facts
        .mutation
        .for_machine(machine.symbol)?
        .state_write_frames
        .iter()
        .find(|frame| frame.state == state.symbol)?
        .frame;
    let expected_frame_path = format!("$P{}", destination.position);
    let exact_frame =
        matches!(frame.complete_paths(), Some([path]) if path == &expected_frame_path);
    // Before provider selection, a boundary-operator initializer makes the
    // source write frame opaque. The selected scalar lane replaces that one
    // initializer with a checked-body call whose complete scalar-only shape is
    // replayed below; the exact two-statement body leaves the destination
    // assignment as its only caller-visible write.
    let unresolved_selected_frame = selected_scalar_result_local.is_some()
        && frame.completeness() == facts::WriteFrameCompleteness::Opaque;
    if !exact_frame && !unresolved_selected_frame {
        return None;
    }
    let value = facts.values.scalar_expressions.expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
    let direct_literal = matches!(value, CheckedScalarExpression::IntegerLiteral { .. })
        || matches!(value, CheckedScalarExpression::IeeeFloatLiteral { .. })
        || matches!(
            value,
            CheckedScalarExpression::Boolean(expression)
                if matches!(
                    expression.as_ref(),
                    checked_trees::CheckedBooleanExpression::Constant(_)
                )
        );
    let direct_parameter = match value {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => Some((*position, *primitive_type)),
        CheckedScalarExpression::Boolean(expression) => match expression.as_ref() {
            checked_trees::CheckedBooleanExpression::Parameter { position } => {
                Some((*position, PrimitiveType::Bool))
            }
            _ => None,
        },
        _ => None,
    };
    let direct_parameter_is_exact = direct_parameter.is_some_and(|(position, primitive_type)| {
        scalar_parameters
            .get(position)
            .is_some_and(|parameter| parameter.primitive_type == primitive_type)
            && *destination_type == primitive_type
    });
    let direct_result_is_exact = matches!(
        (result_local, value),
        (
            Some(result),
            CheckedScalarExpression::Local {
                position: 0,
                primitive_type,
            },
        ) if *primitive_type == result.primitive_type
            && *destination_type == result.primitive_type
            && matches!(
                result.primitive_type,
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
            )
            && scalar_parameters.is_empty()
    );
    if !(direct_literal && scalar_parameters.is_empty()
        || direct_parameter_is_exact
        || direct_result_is_exact)
        || crate::values::scalar_expression_type(value) != Some(*destination_type)
    {
        return None;
    }
    Some(CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
        statement_index,
        destination_parameter_index: 0,
        value: value.clone(),
    })
}

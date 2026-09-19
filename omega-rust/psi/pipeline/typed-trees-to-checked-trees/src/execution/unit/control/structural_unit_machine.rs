//! Building the control machine of one structural unit.

use crate::execution::terminal_unit::{
    BTreeSet, CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedStructuralAccess, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferPlan, CheckedStructuralScalarArgumentPlan,
    CheckedStructuralUnitControlMachinePlan, CheckedStructuralUnitControlStatePlan,
    CheckedStructuralUnitControlTerminatorPlan, Multiplicity, PermissionAccess,
    PermissionEventKind, PermissionEventSource, PrimitiveType, ShapeCollector, StatementNode,
    TransitionExit, TransitionGuardNode, TransitionTargetNode, TypedTrees,
    checked_no_code_affine_discard_positions, is_unit, machine_binders, state_flow,
    structural_scalar_signature,
};
use checked_trees::{
    CheckedStructuralRankedArgumentPlan, CheckedStructuralRankedGuardPlan,
    CheckedStructuralRankedSccEdgePlan, CheckedStructuralRankedSccPlan,
};

/// Bind one closed scalar return to an exact affine structural entry frontier.
/// This is deliberately separate from the primitive scalar graph: structural
/// parameters are custody, not fake scalar arguments.
pub(crate) fn build_structural_unit_control_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<CheckedStructuralUnitControlMachinePlan> {
    let states = program.machine_states(machine);
    if states.len() < 2 {
        return None;
    }
    let proven_ranked_sccs =
        crate::checks::termination::proven_nat_countdown_sccs_with_call_frames(
            program,
            machine,
            call_frames,
        )?;
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
            || parameters.len() + scalar_parameters.len()
                != crate::execution::terminal_unit::abi_parameter_count(
                    program.state_parameters(state),
                )
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
                                source: checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: u32::try_from(source_index).ok()? },
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
                            source: checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: u32::try_from(source_index).ok()? },
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
                                source: checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index: u32::try_from(source_index).ok()? },
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
                                        source: checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: u32::try_from(source_index).ok()? },
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
                                    source: checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: u32::try_from(source_index).ok()? },
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
                                        source: checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index: u32::try_from(source_index)
                                            .ok()? },
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
                            erased_arguments: Vec::new(),
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

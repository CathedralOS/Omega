//! Structural control and boundary-machine construction.

use super::*;

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
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedStructuralUnitControlMachinePlan> {
    let states = program.machine_states(machine);
    if states.len() < 2 {
        return None;
    }
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
            structural_scalar_signature(program, shapes, machine, state, &binders)?;
        let parameters = structural_parameters;
        if parameters.is_empty()
            || parameters.iter().any(|parameter| {
                parameter.is_self
                    || parameter.multiplicity != Multiplicity::Affine
                    || !parameter.qualifications.is_empty()
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
                let target_index = states
                    .iter()
                    .position(|candidate| candidate.symbol == path.symbol)?;
                let (target_parameters, target_scalar_parameters) = &signatures[target_index];
                let arguments = program.statement_table.expression_handles(*arguments);
                if arguments.len() != target_parameters.len() + target_scalar_parameters.len() {
                    return None;
                }
                let mut transferred_sources = BTreeSet::new();
                let transfers = target_parameters
                    .iter()
                    .enumerate()
                    .map(|(target_index, target)| {
                        let argument = arguments.get(target.position as usize)?;
                        let place = crate::flow::canonical_place_from_expression_in_state(
                            program,
                            state.symbol,
                            0,
                            *argument,
                        )?;
                        let psi_facts::PlaceRoot::Symbol(root) = place.root else {
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
                            target_parameter_index: u32::try_from(target_index).ok()?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let scalar_arguments = target_scalar_parameters
                    .iter()
                    .enumerate()
                    .map(|(target_index, target)| {
                        let argument_ordinal = target.source_position;
                        let expression = facts.values.scalar_expressions.expression_at(
                            state.symbol,
                            0,
                            CheckedScalarExpressionRole::TransitionArgument { argument_ordinal },
                        )?;
                        let source_index = match expression {
                            CheckedScalarExpression::Boolean(expression)
                                if target.primitive_type == PrimitiveType::Bool =>
                            {
                                let psi_checked_trees::CheckedBooleanExpression::Parameter {
                                    position,
                                } = expression.as_ref()
                                else {
                                    return None;
                                };
                                *position
                            }
                            CheckedScalarExpression::Parameter {
                                position,
                                primitive_type,
                            } if *primitive_type == target.primitive_type => *position,
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
                if cleanup.target_state != path.symbol {
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
                )?;
                let guard_scalar_parameter_index = match guard_expression {
                    CheckedScalarExpression::Boolean(expression)
                        if matches!(
                            expression.as_ref(),
                            psi_checked_trees::CheckedBooleanExpression::Parameter { .. }
                        ) =>
                    {
                        let psi_checked_trees::CheckedBooleanExpression::Parameter { position } =
                            expression.as_ref()
                        else {
                            unreachable!()
                        };
                        let parameter = source_scalar_parameters.get(*position)?;
                        (parameter.primitive_type == PrimitiveType::Bool)
                            .then(|| u32::try_from(*position).ok())??
                    }
                    _ => return None,
                };
                let build_successor =
                    |statement_ordinal: u32,
                     transition: &psi_typed_trees::statement::TableTransition|
                     -> Option<CheckedStructuralControlSuccessorPlan> {
                        let TransitionTargetNode::Named {
                            path, arguments, ..
                        } = program.statement_table.transition_target(transition.target)
                        else {
                            return None;
                        };
                        let target_index = states
                            .iter()
                            .position(|candidate| candidate.symbol == path.symbol)?;
                        let (target_parameters, target_scalar_parameters) =
                            &signatures[target_index];
                        let arguments = program.statement_table.expression_handles(*arguments);
                        if arguments.len()
                            != target_parameters.len() + target_scalar_parameters.len()
                        {
                            return None;
                        }
                        let mut transferred_sources = BTreeSet::new();
                        let transfers = target_parameters
                            .iter()
                            .enumerate()
                            .map(|(target_index, target)| {
                                let argument = arguments.get(target.position as usize)?;
                                let place = crate::flow::canonical_place_from_expression_in_state(
                                    program,
                                    state.symbol,
                                    usize::try_from(statement_ordinal).ok()?,
                                    *argument,
                                )?;
                                let psi_facts::PlaceRoot::Symbol(root) = place.root else {
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
                                    target_parameter_index: u32::try_from(target_index).ok()?,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?;
                        let scalar_arguments = target_scalar_parameters
                            .iter()
                            .enumerate()
                            .map(|(target_index, target)| {
                                let argument_ordinal = target.source_position;
                                let expression = facts.values.scalar_expressions.expression_at(
                                    state.symbol,
                                    statement_ordinal,
                                    CheckedScalarExpressionRole::TransitionArgument {
                                        argument_ordinal,
                                    },
                                )?;
                                let source_index = match expression {
                                    CheckedScalarExpression::Boolean(expression)
                                        if target.primitive_type == PrimitiveType::Bool =>
                                    {
                                        let psi_checked_trees::CheckedBooleanExpression::Parameter {
                                            position,
                                        } = expression.as_ref()
                                        else {
                                            return None;
                                        };
                                        *position
                                    }
                                    CheckedScalarExpression::Parameter {
                                        position,
                                        primitive_type,
                                    } if *primitive_type == target.primitive_type => *position,
                                    _ => return None,
                                };
                                if source_scalar_parameters
                                    .get(source_index)
                                    .is_none_or(|source| {
                                        source.primitive_type != target.primitive_type
                                    })
                                {
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
                        if cleanup.target_state != path.symbol {
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
    if predecessor_counts[0] != 0
        || predecessor_counts
            .iter()
            .filter(|count| **count == 2)
            .count()
            > 1
    {
        return None;
    }
    Some(CheckedStructuralUnitControlMachinePlan {
        machine: machine.symbol,
        attachment_type_identity: attachment_type_identity?,
        states: checked_states,
    })
}

pub(super) fn build_boundary_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedBoundaryMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let result_type = if is_unit(program, state.return_type) {
        None
    } else {
        Some(program.primitive_type_reference(state.return_type)?)
    };
    if !program
        .statement_table
        .statements(state.statement_nodes)
        .is_empty()
    {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters, scalar_parameters) =
        structural_scalar_signature(program, shapes, machine, state, &binders)?;
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
        attachment_type_identity: Some(attachment_type_identity),
        structural_parameters,
        scalar_parameters,
        result_type,
        domain_requirements,
        contract_fingerprint: contract.fingerprint,
        contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
        service_reach: state_flow.service_reach.clone(),
    })
}

/// Project the narrow static boundary-trait surface used by checked-adapter
/// dispatch. A trait requirement is not an attached machine and therefore
/// contributes no provider value or structural attachment.
pub(super) fn build_static_boundary_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Vec<CheckedBoundaryMachinePlan> {
    let mut plans = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .filter(|definition| program.trait_type_parameters(definition).is_empty())
        .flat_map(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .filter(|signature| {
                    program
                        .state_signature_type_parameters(signature)
                        .is_empty()
                        && program
                            .state_signature_parameters(signature)
                            .iter()
                            .all(|parameter| {
                                !parameter.is_self
                                    && !parameter.is_const
                                    && !parameter.is_mutable
                                    && program
                                        .primitive_type_reference(parameter.type_reference)
                                        .is_some()
                            })
                        && is_unit(program, signature.return_type)
                        && program.state_signature_contracts(signature).is_empty()
                        && !signature.suspends
                        && !signature.blocks
                })
                .filter_map(|signature| {
                    let capsule = facts
                        .contract_plans
                        .crash_capsule(definition.symbol, signature.symbol)?;
                    let call_reaches = facts
                        .flow
                        .control
                        .calls
                        .iter()
                        .map(|(_, call)| call)
                        .filter(|call| call.target_symbol == signature.symbol)
                        .map(|call| call.service_reach.transitive)
                        .collect::<Vec<_>>();
                    let [published_reach, rest @ ..] = call_reaches.as_slice() else {
                        return None;
                    };
                    if rest.iter().any(|reach| reach != published_reach) {
                        return None;
                    }
                    let service_reach = psi_language_semantics::ServiceReachSummary {
                        direct: *published_reach,
                        transitive: *published_reach,
                    };
                    Some(CheckedBoundaryMachinePlan {
                        machine: signature.symbol,
                        state: signature.symbol,
                        attachment_type_identity: None,
                        structural_parameters: Vec::new(),
                        scalar_parameters: program
                            .state_signature_parameters(signature)
                            .iter()
                            .enumerate()
                            .map(|(position, parameter)| {
                                Some(CheckedStructuralScalarParameterPlan {
                                    source_position: u32::try_from(position).ok()?,
                                    primitive_type: program
                                        .primitive_type_reference(parameter.type_reference)?,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?,
                        result_type: None,
                        domain_requirements: Vec::new(),
                        contract_fingerprint: capsule.target_contract_fingerprint(),
                        contract_service_reach: psi_language_semantics::ServiceReachPlan {
                            interface:
                                psi_language_semantics::ServiceReachInterface::PublishedCeiling(
                                    *published_reach,
                                ),
                            checked_inferred: *published_reach,
                        },
                        service_reach,
                    })
                })
        })
        .collect::<Vec<_>>();
    plans.sort_by_key(|plan| (plan.machine.arena_index(), plan.machine.generation()));
    plans.dedup_by_key(|plan| plan.machine);
    plans
}

pub(super) fn build_checked_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedUnitEffectMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if !is_unit(program, state.return_type) {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders)?;
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
    let calls = facts.flow.control.calls.span_or_empty(state_flow.calls);
    let statements = program.statement_table.statements(state.statement_nodes);
    let local_count = statements
        .iter()
        .take_while(|statement| matches!(statement, StatementNode::LocalData(_)))
        .count();
    let local_statements = &statements[..local_count];
    let call_statements = &statements[local_count..];
    if calls.len() != call_statements.len()
        || call_statements
            .iter()
            .any(|statement| !matches!(statement, StatementNode::Call(_)))
    {
        return None;
    }
    let local_rows = build_unit_trivial_affine_locals(
        program,
        facts,
        shapes,
        machine,
        state,
        &binders,
        local_statements,
    )?;
    let trivial_affine_locals = local_rows
        .iter()
        .map(|(plan, _)| plan.clone())
        .collect::<Vec<_>>();
    let admitted_local_symbols = local_rows
        .iter()
        .map(|(_, symbol)| *symbol)
        .collect::<Vec<_>>();

    let mut operations = trivial_affine_locals
        .iter()
        .map(
            |local| CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: local.declaration_ordinal,
                declaration_ordinal: local.declaration_ordinal,
                type_identity: local.type_identity.clone(),
            },
        )
        .collect::<Vec<_>>();
    operations.reserve(calls.len() + 1);
    for (call_index, call) in calls.iter().enumerate() {
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
            &entry_claims,
            call,
            false,
            None,
        )?);
    }
    operations.push(CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index: u32::try_from(statements.len()).ok()?,
        trivial_affine_local_discard_ordinals: (0..trivial_affine_locals.len())
            .rev()
            .map(|ordinal| u32::try_from(ordinal).ok())
            .collect::<Option<Vec<_>>>()?,
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

    Some(CheckedUnitEffectMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        trivial_affine_locals,
        entry_claims,
        body_qualifications,
        contract_fingerprint: contract.fingerprint,
        contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
        service_reach: state_flow.service_reach.clone(),
        operations,
    })
}

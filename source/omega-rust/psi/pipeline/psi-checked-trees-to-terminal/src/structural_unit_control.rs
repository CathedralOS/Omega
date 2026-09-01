//! Structural Unit control lowering.
//!
//! This module owns checked multi-state control, edge-local transfer cleanup,
//! and exact join-frontier publication for Unit-result machines.

use super::*;

pub(super) fn lower_structural_unit_control_machine(
    checked: &CheckedTrees,
    plan: &CheckedStructuralUnitControlMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if plan.ranked_scc.is_some() {
        return lower_ranked_structural_unit_countdown(checked, plan);
    }
    if plan.states.len() < 2 {
        return unsupported("structural Unit control plan must contain multiple states");
    }
    let (structural_types, type_ids) = lower_structural_type_plans(
        &checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .structural_types,
    )?;
    if plan
        .states
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
        return unsupported(
            "structural Unit control supports at most two checked conditional states",
        );
    }
    for state in &plan.states {
        if state.structural_parameters.is_empty() {
            return unsupported("structural Unit state has no structural parameters");
        }
        let mut positions = BTreeSet::new();
        for parameter in &state.structural_parameters {
            if parameter.is_self
                || parameter.multiplicity != Multiplicity::Affine
                || !parameter.qualifications.is_empty()
                || !positions.insert(parameter.position)
            {
                return unsupported(
                    "structural Unit state signature is not claim-free affine custody",
                );
            }
            lookup_type_id(&type_ids, &parameter.type_identity)?;
        }
        for parameter in &state.scalar_parameters {
            if !positions.insert(parameter.source_position) {
                return unsupported(
                    "structural Unit scalar inputs overlap the authored parameter partition",
                );
            }
            terminal_scalar_type(parameter.primitive_type)?;
        }
        if positions.len() != state.structural_parameters.len() + state.scalar_parameters.len()
            || positions
                .iter()
                .copied()
                .enumerate()
                .any(|(index, position)| u32::try_from(index).ok() != Some(position))
        {
            return unsupported(
                "structural Unit parameter maps do not partition authored positions",
            );
        }
        match &state.terminator {
            CheckedStructuralUnitControlTerminatorPlan::Conditional {
                guard_scalar_parameter_index,
                ..
            } if usize::try_from(*guard_scalar_parameter_index)
                .ok()
                .and_then(|index| state.scalar_parameters.get(index))
                .is_some_and(|parameter| parameter.primitive_type == PrimitiveType::Bool) => {}
            CheckedStructuralUnitControlTerminatorPlan::Conditional { .. } => {
                return unsupported(
                    "structural Unit conditional must select one Boolean scalar state input",
                );
            }
            _ => {}
        }
    }
    let mut next_place = 1_u64;
    let entry_parameters = lower_unit_parameters(
        &plan.states[0].structural_parameters,
        &type_ids,
        &[],
        &mut next_place,
    )?;
    if entry_parameters.is_empty() {
        return unsupported("structural Unit control entry has no structural parameters");
    }
    let mut next_value = 1_u64;
    let state_scalar_parameters = plan
        .states
        .iter()
        .map(|state| {
            state
                .scalar_parameters
                .iter()
                .map(|parameter| {
                    Ok(ValueDeclaration {
                        id: value_id(allocate_dense(&mut next_value)?),
                        scalar_type: terminal_scalar_type(parameter.primitive_type)?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let entry_places = entry_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<Vec<_>>();
    let entry_place_order = entry_places
        .iter()
        .enumerate()
        .map(|(index, place)| (*place, index))
        .collect::<BTreeMap<_, _>>();
    let state_ids = plan
        .states
        .iter()
        .enumerate()
        .map(|(index, state)| Ok((state.state, block_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if state_ids
        .iter()
        .enumerate()
        .any(|(index, (state, _))| state_ids[..index].iter().any(|(other, _)| other == state))
    {
        return unsupported("structural Unit control plan contains duplicate states");
    }

    let mut predecessor_counts = vec![0_usize; plan.states.len()];
    for state in &plan.states {
        let targets = match &state.terminator {
            CheckedStructuralUnitControlTerminatorPlan::ReturnUnit { .. } => Vec::new(),
            CheckedStructuralUnitControlTerminatorPlan::Jump { target_state, .. } => {
                vec![*target_state]
            }
            CheckedStructuralUnitControlTerminatorPlan::Conditional {
                when_true,
                when_false,
                ..
            } => {
                if when_true.target_state == when_false.target_state {
                    return unsupported(
                        "structural Unit conditional successors must remain distinct",
                    );
                }
                vec![when_true.target_state, when_false.target_state]
            }
        };
        for target in targets {
            let target_index = plan
                .states
                .iter()
                .position(|candidate| candidate.state == target)
                .ok_or(LoweringError::Unsupported(
                    "structural Unit jump targets an unknown checked state",
                ))?;
            predecessor_counts[target_index] += 1;
            if predecessor_counts[target_index] > 2 {
                return unsupported("structural Unit join supports exactly two incoming frontiers");
            }
        }
    }
    if predecessor_counts[0] != 0 {
        return unsupported("structural Unit control entry has an incoming edge");
    }
    if predecessor_counts
        .iter()
        .filter(|count| **count == 2)
        .count()
        > 1
    {
        return unsupported("structural Unit control supports at most one join state");
    }

    let mut bindings = vec![None; plan.states.len()];
    bindings[0] = Some(entry_places);
    let mut received_predecessors = vec![0_usize; plan.states.len()];
    let mut completed = BTreeSet::new();
    loop {
        let Some(index) = (0..plan.states.len()).find(|index| {
            bindings[*index].is_some()
                && !completed.contains(index)
                && (*index == 0 || received_predecessors[*index] == predecessor_counts[*index])
        }) else {
            break;
        };
        completed.insert(index);
        let source = bindings[index]
            .as_ref()
            .expect("ready structural state has a binding")
            .clone();
        if source.len() != plan.states[index].structural_parameters.len() {
            return unsupported("structural Unit state binding has the wrong arity");
        }
        let successors = match &plan.states[index].terminator {
            CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                trivial_affine_discard_parameter_positions,
            } => {
                let expected = plan.states[index]
                    .structural_parameters
                    .iter()
                    .rev()
                    .map(|parameter| parameter.position)
                    .collect::<Vec<_>>();
                if *trivial_affine_discard_parameter_positions != expected {
                    return unsupported(
                        "structural Unit return cleanup does not consume its exact frontier",
                    );
                }
                continue;
            }
            CheckedStructuralUnitControlTerminatorPlan::Jump {
                target_state,
                transfers,
                scalar_arguments,
                trivial_affine_discard_parameter_positions,
                ..
            } => vec![(
                target_state,
                transfers.as_slice(),
                scalar_arguments.as_slice(),
                trivial_affine_discard_parameter_positions.as_slice(),
            )],
            CheckedStructuralUnitControlTerminatorPlan::Conditional {
                when_true,
                when_false,
                ..
            } => vec![
                (
                    &when_true.target_state,
                    when_true.transfers.as_slice(),
                    when_true.scalar_arguments.as_slice(),
                    when_true
                        .trivial_affine_discard_parameter_positions
                        .as_slice(),
                ),
                (
                    &when_false.target_state,
                    when_false.transfers.as_slice(),
                    when_false.scalar_arguments.as_slice(),
                    when_false
                        .trivial_affine_discard_parameter_positions
                        .as_slice(),
                ),
            ],
        };
        for (target_state, transfers, scalar_arguments, cleanup_positions) in successors {
            let target_state_index = plan
                .states
                .iter()
                .position(|state| state.state == *target_state)
                .ok_or(LoweringError::Unsupported(
                    "structural Unit jump targets an unknown checked state",
                ))?;
            if completed.contains(&target_state_index) {
                return unsupported("structural Unit control graph contains a cycle");
            }
            let target_arity = plan.states[target_state_index].structural_parameters.len();
            if transfers.len() != target_arity {
                return unsupported(
                    "structural Unit transfer map does not fill its target frontier",
                );
            }
            let target_scalar_parameters = &plan.states[target_state_index].scalar_parameters;
            if scalar_arguments.len() != target_scalar_parameters.len() {
                return unsupported(
                    "structural Unit scalar successor map does not fill its target signature",
                );
            }
            for (target_index, (argument, target_parameter)) in scalar_arguments
                .iter()
                .zip(target_scalar_parameters)
                .enumerate()
            {
                let source_index = usize::try_from(argument.source_scalar_parameter_index)
                    .map_err(|_| {
                        LoweringError::Unsupported(
                            "structural Unit scalar successor source exceeds usize",
                        )
                    })?;
                if argument.target_scalar_parameter_index
                    != u32::try_from(target_index).map_err(|_| {
                        LoweringError::Unsupported(
                            "structural Unit scalar successor target exceeds u32",
                        )
                    })?
                    || argument.argument_ordinal != target_parameter.source_position
                    || argument.primitive_type != target_parameter.primitive_type
                    || plan.states[index]
                        .scalar_parameters
                        .get(source_index)
                        .is_none_or(|source| {
                            source.primitive_type != target_parameter.primitive_type
                        })
                {
                    return unsupported(
                        "structural Unit scalar successor map changes its checked signature",
                    );
                }
            }
            let mut target = vec![None; target_arity];
            let mut used_sources = BTreeSet::new();
            for transfer in transfers {
                let source_index =
                    usize::try_from(transfer.source_parameter_index).map_err(|_| {
                        LoweringError::Unsupported("structural Unit source parameter exceeds usize")
                    })?;
                let target_parameter_index = usize::try_from(transfer.target_parameter_index)
                    .map_err(|_| {
                        LoweringError::Unsupported("structural Unit target parameter exceeds usize")
                    })?;
                let place = *source.get(source_index).ok_or(LoweringError::Unsupported(
                    "structural Unit transfer names an unknown source parameter",
                ))?;
                let source_parameter = &plan.states[index].structural_parameters[source_index];
                let target_parameter = plan.states[target_state_index]
                    .structural_parameters
                    .get(target_parameter_index)
                    .ok_or(LoweringError::Unsupported(
                        "structural Unit transfer names an unknown target parameter",
                    ))?;
                if source_parameter.type_identity != target_parameter.type_identity
                    || source_parameter.multiplicity != target_parameter.multiplicity
                    || source_parameter.qualifications != target_parameter.qualifications
                {
                    return unsupported(
                        "structural Unit transfer changes its checked structural signature",
                    );
                }
                let slot =
                    target
                        .get_mut(target_parameter_index)
                        .ok_or(LoweringError::Unsupported(
                            "structural Unit transfer names an unknown target parameter",
                        ))?;
                if slot.replace(place).is_some() || !used_sources.insert(source_index) {
                    return unsupported("structural Unit transfer map is not one-to-one");
                }
            }
            let expected_cleanup = plan.states[index]
                .structural_parameters
                .iter()
                .enumerate()
                .rev()
                .filter_map(|(parameter_index, parameter)| {
                    (!used_sources.contains(&parameter_index)).then_some(parameter.position)
                })
                .collect::<Vec<_>>();
            if *cleanup_positions != expected_cleanup {
                return unsupported(
                    "structural Unit jump transfer and cleanup do not partition its exact frontier",
                );
            }
            let target = target.into_iter().collect::<Option<Vec<_>>>().ok_or(
                LoweringError::Unsupported(
                    "structural Unit transfer map leaves a target parameter unbound",
                ),
            )?;
            if target
                .windows(2)
                .any(|pair| entry_place_order[&pair[0]] >= entry_place_order[&pair[1]])
            {
                return unsupported(
                    "structural Unit target frontier reorders entry custody outside terminal representation",
                );
            }
            if bindings[target_state_index]
                .as_ref()
                .is_some_and(|existing| existing != &target)
            {
                return unsupported(
                    "structural Unit join predecessors reconstruct different custody frontiers",
                );
            }
            bindings[target_state_index].get_or_insert(target);
            received_predecessors[target_state_index] += 1;
        }
    }
    if bindings.iter().any(Option::is_none) || completed.len() != plan.states.len() {
        return unsupported("structural Unit control graph is cyclic or unreachable");
    }

    let mut next_edge = 1_u64;
    let mut blocks = Vec::with_capacity(plan.states.len());
    for (index, state) in plan.states.iter().enumerate() {
        let state_binding = bindings[index]
            .as_ref()
            .expect("every structural state binding was reconstructed");
        let lower_discards = |positions: &[u32]| -> Result<Vec<PlaceId>, LoweringError> {
            positions
                .iter()
                .map(|position| {
                    let parameter_index = state
                        .structural_parameters
                        .iter()
                        .position(|parameter| parameter.position == *position)
                        .ok_or(LoweringError::Unsupported(
                            "structural Unit cleanup position is absent from its state signature",
                        ))?;
                    state_binding
                        .get(parameter_index)
                        .copied()
                        .ok_or(LoweringError::Unsupported(
                            "structural Unit cleanup position has no live entry binding",
                        ))
                })
                .collect()
        };
        let edge = edge_id(allocate_dense(&mut next_edge)?);
        let terminator = match &state.terminator {
            CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                trivial_affine_discard_parameter_positions,
            } => Terminator::ReturnUnit {
                edge,
                trivial_affine_discards: lower_discards(
                    trivial_affine_discard_parameter_positions,
                )?,
            },
            CheckedStructuralUnitControlTerminatorPlan::Jump {
                statement_ordinal,
                target_state,
                scalar_arguments,
                trivial_affine_discard_parameter_positions,
                ..
            } => {
                if *statement_ordinal != 0 {
                    return unsupported(
                        "structural Unit jump is not the state's sole checked statement",
                    );
                }
                Terminator::Jump {
                    edge,
                    target: state_ids
                        .iter()
                        .find_map(|(state, block)| (*state == *target_state).then_some(*block))
                        .ok_or(LoweringError::Unsupported(
                            "structural Unit jump target has no terminal block",
                        ))?,
                    arguments: scalar_arguments
                        .iter()
                        .map(|argument| {
                            state_scalar_parameters[index]
                                .get(
                                    usize::try_from(argument.source_scalar_parameter_index)
                                        .map_err(|_| {
                                            LoweringError::Unsupported(
                                                "structural Unit scalar successor source exceeds usize",
                                            )
                                        })?,
                                )
                                .map(|parameter| parameter.id)
                                .ok_or(LoweringError::Unsupported(
                                    "structural Unit scalar successor names an unknown source",
                                ))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    trivial_affine_discards: lower_discards(
                        trivial_affine_discard_parameter_positions,
                    )?,
                }
            }
            CheckedStructuralUnitControlTerminatorPlan::Conditional {
                guard_scalar_parameter_index,
                when_true,
                when_false,
            } => {
                if when_true.statement_ordinal != 0 || when_false.statement_ordinal != 1 {
                    return unsupported(
                        "structural Unit conditional successors are not in canonical order",
                    );
                }
                let source_scalar_parameters = &state_scalar_parameters[index];
                let condition = source_scalar_parameters
                    .get(usize::try_from(*guard_scalar_parameter_index).map_err(|_| {
                        LoweringError::Unsupported(
                            "structural Unit guard scalar index exceeds usize",
                        )
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "structural Unit conditional names an unknown scalar guard",
                    ))?;
                let lower_successor =
                    |successor: &psi_checked_trees::CheckedStructuralControlSuccessorPlan,
                     edge: EdgeId|
                     -> Result<SuccessorEdge, LoweringError> {
                        Ok(SuccessorEdge {
                            edge,
                            target: state_ids
                                .iter()
                                .find_map(|(state, block)| {
                                    (*state == successor.target_state).then_some(*block)
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "structural Unit conditional target has no terminal block",
                                ))?,
                            arguments: successor
                                .scalar_arguments
                                .iter()
                                .map(|argument| {
                                    source_scalar_parameters
                                        .get(
                                            usize::try_from(
                                                argument.source_scalar_parameter_index,
                                            )
                                            .map_err(|_| {
                                                LoweringError::Unsupported(
                                                    "structural Unit scalar successor source exceeds usize",
                                                )
                                            })?,
                                        )
                                        .map(|parameter| parameter.id)
                                        .ok_or(LoweringError::Unsupported(
                                            "structural Unit scalar successor names an unknown source",
                                        ))
                                })
                                .collect::<Result<Vec<_>, _>>()?,
                            trivial_affine_discards: lower_discards(
                                &successor.trivial_affine_discard_parameter_positions,
                            )?,
                        })
                    };
                let false_edge = edge_id(allocate_dense(&mut next_edge)?);
                Terminator::Conditional {
                    condition: condition.id,
                    when_true: lower_successor(when_true, edge)?,
                    when_false: lower_successor(when_false, false_edge)?,
                }
            }
        };
        blocks.push(Block {
            id: state_ids[index].1,
            parameters: if index == 0 {
                Vec::new()
            } else {
                state_scalar_parameters[index].clone()
            },
            operations: Vec::new(),
            terminator,
        });
    }
    let machine = TerminalMachine {
        id: machine_id(1),
        attachment: Some(lookup_type_id(&type_ids, &plan.attachment_type_identity)?),
        parameters: state_scalar_parameters[0].clone(),
        structural_parameters: entry_parameters.clone(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: entry_parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
            })
            .collect(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: state_ids[0].1,
        blocks,
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    Ok(LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine.id,
            structural_types,
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![machine],
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences: Vec::new(),
        selected_ieee_float_fma_occurrences: Vec::new(),
    })
}

fn lower_ranked_structural_unit_countdown(
    checked: &CheckedTrees,
    plan: &CheckedStructuralUnitControlMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let Some(ranked) = &plan.ranked_scc else {
        return unsupported("ranked structural control is missing its component");
    };
    let [header_plan, done_plan] = plan.states.as_slice() else {
        return unsupported("ranked structural Unit countdown requires header and exit states");
    };
    if header_plan.state != ranked.header_state
        || ranked.covered_cyclic_edges.len() != 1
        || ranked.rank_lower_bound != 0
        || header_plan.scalar_parameters.len() != 1
        || done_plan.scalar_parameters.len() != 0
    {
        return unsupported("ranked structural Unit countdown component shape");
    }
    let rank_index = usize::try_from(ranked.rank_scalar_parameter_index)
        .map_err(|_| LoweringError::Unsupported("ranked scalar index exceeds usize"))?;
    let Some(rank_parameter_plan) = header_plan.scalar_parameters.get(rank_index) else {
        return unsupported("ranked scalar parameter is absent from the header");
    };
    let rank_scalar_type = terminal_scalar_type(ranked.rank_primitive_type)?;
    let ScalarType::Integer(rank_type) = rank_scalar_type else {
        return unsupported("ranked structural Unit carrier is not an integer");
    };
    if rank_parameter_plan.primitive_type != ranked.rank_primitive_type
        || rank_type.sign() != IntegerSign::Unsigned
        || ranked.rank_upper_bound
            != match rank_type.maximum_value() {
                IntegerValue::Unsigned(value) => value,
                IntegerValue::Signed(_) => return unsupported("ranked carrier is not unsigned"),
            }
    {
        return unsupported("ranked structural Unit carrier bounds");
    }

    let CheckedStructuralUnitControlTerminatorPlan::Conditional {
        guard_scalar_parameter_index,
        when_true,
        when_false,
    } = &header_plan.terminator
    else {
        return unsupported("ranked structural Unit header is not conditional");
    };
    let CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
        trivial_affine_discard_parameter_positions,
    } = &done_plan.terminator
    else {
        return unsupported("ranked structural Unit exit does not return Unit");
    };
    if *guard_scalar_parameter_index != ranked.rank_scalar_parameter_index
        || when_true.target_state != header_plan.state
        || when_false.target_state != done_plan.state
        || !when_true
            .trivial_affine_discard_parameter_positions
            .is_empty()
        || !when_false
            .trivial_affine_discard_parameter_positions
            .is_empty()
        || when_true.scalar_arguments.len() != 1
        || !when_false.scalar_arguments.is_empty()
        || trivial_affine_discard_parameter_positions
            != &done_plan
                .structural_parameters
                .iter()
                .rev()
                .filter(|parameter| !parameter.is_self)
                .map(|parameter| parameter.position)
                .collect::<Vec<_>>()
    {
        return unsupported("ranked structural Unit successor/cleanup shape");
    }
    let [covered] = ranked.covered_cyclic_edges.as_slice() else {
        return unsupported("ranked structural Unit requires one covered edge");
    };
    if covered.source_state != header_plan.state
        || covered.target_state != header_plan.state
        || covered.statement_ordinal != when_true.statement_ordinal
    {
        return unsupported("ranked structural Unit covered edge coordinate");
    }
    let psi_checked_trees::CheckedStructuralRankedGuardPlan::UnsignedParameterPositive {
        scalar_parameter_index,
        primitive_type,
    } = covered.guard;
    let psi_checked_trees::CheckedStructuralRankedArgumentPlan::UnsignedParameterMinusOne {
        argument_ordinal,
        source_scalar_parameter_index,
        target_scalar_parameter_index,
        primitive_type: argument_type,
    } = covered.successor_argument;
    if scalar_parameter_index != ranked.rank_scalar_parameter_index
        || primitive_type != ranked.rank_primitive_type
        || source_scalar_parameter_index != ranked.rank_scalar_parameter_index
        || target_scalar_parameter_index != ranked.rank_scalar_parameter_index
        || argument_type != ranked.rank_primitive_type
        || argument_ordinal != rank_parameter_plan.source_position
        || when_true.scalar_arguments[0].argument_ordinal != argument_ordinal
        || when_true.scalar_arguments[0].source_scalar_parameter_index
            != source_scalar_parameter_index
        || when_true.scalar_arguments[0].target_scalar_parameter_index
            != target_scalar_parameter_index
        || when_true.scalar_arguments[0].primitive_type != argument_type
    {
        return unsupported("ranked structural Unit guard/argument evidence");
    }

    validate_ranked_structural_transfers(header_plan, header_plan, when_true)?;
    validate_ranked_structural_transfers(header_plan, done_plan, when_false)?;
    let (structural_types, type_ids) = lower_structural_type_plans(
        &checked
            .facts
            .flow
            .terminal_structural_unit_controls
            .structural_types,
    )?;
    let mut next_place = 1_u64;
    let structural_parameters = lower_unit_parameters(
        &header_plan.structural_parameters,
        &type_ids,
        &[],
        &mut next_place,
    )?;
    let initial = value_id(1);
    let rank = value_id(2);
    let zero = value_id(3);
    let condition = value_id(4);
    let one = value_id(5);
    let next = value_id(6);
    let preheader = block_id(1);
    let header = block_id(2);
    let decrement = block_id(3);
    let done = block_id(4);
    let preheader_edge = edge_id(1);
    let guard_edge = edge_id(2);
    let exit_edge = edge_id(3);
    let backedge = edge_id(4);
    let return_edge = edge_id(5);
    let rank_declaration = ValueDeclaration {
        id: rank,
        scalar_type: rank_scalar_type,
    };
    let machine = TerminalMachine {
        id: machine_id(1),
        attachment: Some(lookup_type_id(&type_ids, &plan.attachment_type_identity)?),
        parameters: vec![ValueDeclaration {
            id: initial,
            scalar_type: rank_scalar_type,
        }],
        structural_parameters: structural_parameters.clone(),
        ranked_scc: Some(TerminalRankedScc {
            header,
            rank_parameter: rank,
            rank_type,
            lower_bound: IntegerValue::Unsigned(ranked.rank_lower_bound),
            upper_bound: IntegerValue::Unsigned(ranked.rank_upper_bound),
            covered_cyclic_edges: vec![TerminalRankedSccEdge {
                edge: backedge,
                source: decrement,
                target: header,
                guard: TerminalRankedGuard::UnsignedParameterPositive {
                    block: header,
                    edge: guard_edge,
                    condition,
                    parameter: rank,
                },
                successor_argument: TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
                    argument_index: ranked.rank_scalar_parameter_index,
                    argument: next,
                    source_parameter: rank,
                    target_parameter: rank,
                },
            }],
        }),
        result: TerminalMachineResult::Unit,
        structural_places: structural_parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
            })
            .collect(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: preheader,
        blocks: vec![
            Block {
                id: preheader,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Jump {
                    edge: preheader_edge,
                    target: header,
                    arguments: vec![initial],
                    trivial_affine_discards: Vec::new(),
                },
            },
            Block {
                id: header,
                parameters: vec![rank_declaration],
                operations: vec![
                    Operation {
                        id: operation_id(1),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: zero,
                            scalar_type: rank_scalar_type,
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(0),
                        },
                    },
                    Operation {
                        id: operation_id(2),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: condition,
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::IntegerLessThan {
                            left: zero,
                            right: rank,
                        },
                    },
                ],
                terminator: Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: guard_edge,
                        target: decrement,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: exit_edge,
                        target: done,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            },
            Block {
                id: decrement,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: operation_id(3),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: one,
                            scalar_type: rank_scalar_type,
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(1),
                        },
                    },
                    Operation {
                        id: operation_id(4),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: next,
                            scalar_type: rank_scalar_type,
                        }),
                        kind: OperationKind::ExactIntegerSubtract {
                            left: rank,
                            right: one,
                            obligation: obligation_id(1),
                        },
                    },
                ],
                terminator: Terminator::Jump {
                    edge: backedge,
                    target: header,
                    arguments: vec![next],
                    trivial_affine_discards: Vec::new(),
                },
            },
            Block {
                id: done,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: return_edge,
                    trivial_affine_discards: structural_parameters
                        .iter()
                        .rev()
                        .filter(|parameter| !parameter.is_self)
                        .map(|parameter| parameter.place)
                        .collect(),
                },
            },
        ],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let mut lowered = LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine.id,
            structural_types,
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![machine],
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences: Vec::new(),
        selected_ieee_float_fma_occurrences: Vec::new(),
    };
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

fn validate_ranked_structural_transfers(
    source: &psi_checked_trees::CheckedStructuralUnitControlStatePlan,
    target: &psi_checked_trees::CheckedStructuralUnitControlStatePlan,
    successor: &psi_checked_trees::CheckedStructuralControlSuccessorPlan,
) -> Result<(), LoweringError> {
    if successor.transfers.len() != target.structural_parameters.len()
        || successor
            .transfers
            .iter()
            .enumerate()
            .any(|(index, transfer)| {
                usize::try_from(transfer.source_parameter_index).ok() != Some(index)
                    || usize::try_from(transfer.target_parameter_index).ok() != Some(index)
                    || source.structural_parameters.get(index)
                        != target.structural_parameters.get(index)
            })
    {
        return unsupported("ranked structural Unit transfer changes its custody frontier");
    }
    Ok(())
}

//! Emit every authored state using shared catalogs and simultaneous typed edges.

use super::*;

pub(in crate::attached_unit::composed_control) fn emit(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    admitted: AdmittedGraph<'_>,
    terminal_machine: MachineId,
    parameters: Vec<StructuralParameterDeclaration>,
    scalar_parameters: Vec<ValueDeclaration>,
    catalogs: &mut catalogs::ComposedCatalogs,
) -> Result<(TerminalMachine, Vec<LoweredSourceCallOccurrence>), LoweringError> {
    let result_places_start = catalogs.result_places.len();
    let mut structural_places = parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        })
        .collect::<Vec<_>>();
    let machine_result = returns::result(&plan.result, catalogs, &mut structural_places)?;
    let entry = &plan.states[0];
    let claims = crate::attached_unit::claims::lower_unit_entry_claims(
        plan.machine,
        entry.state,
        &entry.entry_claims,
        &parameters,
    )?;
    let content_entry_claims = crate::content_conservation::lower_whole_content_entry_claims(
        checked,
        &catalogs.structural_types,
        &entry.structural_parameters,
        &parameters,
        &entry.entry_claims,
        &claims.source_claims,
    )?;
    let content_identity_reshuffles = crate::attached_unit::claims::lower_result_identity(
        checked,
        plan.machine,
        entry.state,
        &entry.structural_parameters,
        &parameters,
        &machine_result,
        &claims.source_claims,
    )?;
    let entry_reentered = plan
        .states
        .iter()
        .flat_map(successors)
        .any(|successor| successor.target_state == plan.states[0].state);
    // Invocation parameters are immutable. Reentering the authored entry
    // therefore uses an ordinary parameterized block behind a one-shot jump.
    let invocation_entry = if entry_reentered {
        Some(block_id(allocate_dense(&mut catalogs.next_block)?))
    } else {
        None
    };
    let mut state_ids = Vec::new();
    let mut state_views = Vec::new();
    let mut state_values = Vec::new();
    for (position, state) in plan.states.iter().enumerate() {
        state_ids.push(block_id(allocate_dense(&mut catalogs.next_block)?));
        if position != 0 || entry_reentered {
            let mut block_parameters = lower_unit_parameters(
                &state.structural_parameters,
                &catalogs.type_ids,
                &catalogs.domain_ids,
                &mut catalogs.next_place,
            )?;
            let mut block_position = 0_u32;
            for parameter in &mut block_parameters {
                if parameter.is_self {
                    *parameter = parameters
                        .iter()
                        .find(|entry| entry.is_self)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph receiver invocation is missing",
                        ))?
                        .clone();
                } else {
                    // The persistent receiver is not a block parameter. Only
                    // transferred structural values occupy its dense namespace.
                    parameter.position = block_position;
                    block_position =
                        block_position
                            .checked_add(1)
                            .ok_or(LoweringError::Unsupported(
                                "Unit graph block parameter count exceeds u32",
                            ))?;
                }
            }
            structural_places.extend(
                block_parameters
                    .iter()
                    .filter(|parameter| !parameter.is_self)
                    .map(|parameter| StructuralPlaceDeclaration {
                        id: parameter.place,
                        kind: StructuralPlaceKind::BlockParameter {
                            block: state_ids[position],
                            position: parameter.position,
                        },
                    }),
            );
            state_views.push(block_parameters);
            state_values.push(
                state
                    .scalar_parameters
                    .iter()
                    .map(|parameter| {
                        Ok(ValueDeclaration {
                            qualifications: Default::default(),
                            id: value_id(allocate_dense(&mut catalogs.next_value)?),
                            scalar_type: terminal_scalar_type(parameter.primitive_type)?,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?,
            );
        } else {
            state_views.push(parameters.clone());
            state_values.push(scalar_parameters.clone());
        }
    }
    let mut blocks = Vec::new();
    if let Some(entry) = invocation_entry {
        blocks.push(Block {
            id: entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Jump {
                edge: edge_id(allocate_dense(&mut catalogs.next_edge)?),
                target: state_ids[0],
                arguments: scalar_parameters
                    .iter()
                    .map(|parameter| parameter.id)
                    .collect(),
                structural_arguments: parameters
                    .iter()
                    .filter(|parameter| !parameter.is_self)
                    .map(|parameter| StructuralArgument {
                        place: parameter.place,
                        path: Vec::new(),
                        access: parameter.access,
                    })
                    .collect(),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            },
        });
    }
    let mut occurrences = Vec::new();
    let mut block_ranks = std::collections::BTreeMap::new();
    let mut rank_edges = std::collections::BTreeMap::new();
    for (position, state) in plan.states.iter().enumerate() {
        let state_parameters = state_views[position]
            .iter()
            .zip(&state.structural_parameters)
            .map(|(parameter, source)| {
                // Source readers use authored positions in mixed signatures.
                let mut parameter = parameter.clone();
                parameter.position = source.position;
                parameter
            })
            .collect::<Vec<_>>();
        let mut operations = OperationBuffer::new(catalogs.next_operation - 1);
        let mut evaluation = crate::attached_unit::argument_evaluation::Evaluation {
            structural_value_owners: Vec::new(),
            selection_cleanups: Vec::new(),
            structural_locals: Vec::new(),
            local_cases: Vec::new(),
            record_fields: crate::scalar_computations::fields::prepare(
                checked,
                plan.machine,
                &catalogs.structural_types,
            )?,
            arrays: crate::scalar_computations::arrays::prepare(
                checked,
                plan.machine,
                &catalogs.structural_types,
                &mut catalogs.next_place,
            )?,
            cases: crate::scalar_computations::cases::prepare(
                checked,
                plan.machine,
                &catalogs.structural_types,
                &mut catalogs.next_place,
            )?,
            primitive_storage: Vec::new(),
            scalar_bindings: None,
            structural_fields: Vec::new(),
            structural_cases: Vec::new(),
            structural_parameters: state
                .structural_parameters
                .iter()
                .zip(&state_parameters)
                .map(|(source, parameter)| (source.position, parameter.clone()))
                .collect(),
            entry: state_ids[position],
            block_structural_parameters: Vec::new(),
            current: state_ids[position],
            parameters: if position == 0 && !entry_reentered {
                Vec::new()
            } else {
                state_values[position].clone()
            },
            operation_start: 0,
            blocks: Vec::new(),
        };
        let mut values = state_values[position].clone();
        let mut next_value = catalogs.next_value;
        let mut next_block = catalogs.next_block;
        let mut next_edge = catalogs.next_edge;
        let current_rank =
            if let Some(scalar_position) = ranking::scalar_parameter_position(plan, state) {
                Some(values[scalar_position].id)
            } else {
                ranking::parameter_position(plan, state).map(|parameter_position| {
                    crate::operation_emission::emit_byte_length(
                        state_parameters[parameter_position].place,
                        &mut next_value,
                        &mut operations,
                    )
                })
            };
        let bindings = scalars::emit_prefix(
            checked,
            state,
            &evaluation.structural_parameters,
            &catalogs.structural_types,
            &mut values,
            &mut next_value,
            &mut operations,
        )?;
        evaluation.scalar_bindings = Some(bindings.clone());
        evaluation.structural_fields =
            crate::scalar_bindings::StructuralScalarFieldBinding::collect(
                &evaluation.structural_parameters,
                &catalogs.structural_types,
            );
        evaluation.structural_cases =
            crate::scalar_bindings::structural_cases::StructuralCaseBinding::collect(
                &evaluation.structural_parameters,
                &catalogs.structural_types,
            );
        super::super::emission::emit_call_operations(
            checked,
            plan.machine,
            state,
            catalogs,
            &state_parameters,
            &claims.source_claims,
            &mut evaluation,
            &mut values,
            &mut next_value,
            &mut next_block,
            &mut next_edge,
            &mut operations,
        )?;
        let bindings = evaluation
            .scalar_bindings
            .clone()
            .ok_or(LoweringError::Unsupported(
                "graph body lost its scalar namespace",
            ))?;
        let condition =
            if let CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
                &state.terminator
            {
                let expression = bindings.expression_at(
                    checked,
                    state.state,
                    when_true.statement_ordinal,
                    CheckedScalarExpressionRole::Guard,
                )?;
                if expression.scalar_type() != ScalarType::Boolean
                    || direct_expression_contains_short_circuit(&expression)
                {
                    return unsupported("Unit graph guard needs a branch-free Boolean value");
                }
                validate_direct_parameter_types(
                    &expression,
                    &values
                        .iter()
                        .map(|value| value.scalar_type)
                        .collect::<Vec<_>>(),
                )?;
                Some(emit_direct_expression(
                    &expression,
                    &values,
                    &mut next_value,
                    &mut operations,
                ))
            } else {
                None
            };
        let body_end = operations.len();
        let prepared_cases = case_emission::prepare(
            state,
            catalogs,
            &state_parameters,
            &operations,
            &mut next_value,
        )?;
        let returned_case = returns::emit(
            checked,
            state,
            &machine_result,
            &bindings,
            catalogs,
            &values,
            &mut next_value,
            &mut operations,
        )?;
        let inherited_lengths = operations.byte_lengths.clone();
        let mut edge_blocks = Vec::new();
        let mut successor = |edge: &CheckedStructuralControlSuccessorPlan,
                             payload_values: &[(u32, ValueDeclaration)],
                             case_edge: bool|
         -> Result<SuccessorEdge, LoweringError> {
            // Case dispatch consumes its subject separately. Ordinary edges
            // retain their exact local remainder until selected operands finish.
            let trivial_affine_discards = if case_edge {
                Vec::new()
            } else {
                result_custody::successor_discards(
                    checked,
                    plan.machine,
                    &admitted.source_states[position],
                    state,
                    edge,
                )?
                .into_iter()
                .map(|ordinal| {
                    let result = case_emission::result(state, ordinal, &operations)?;
                    Ok(evaluation.current_structural_place(result.place))
                })
                .collect::<Result<Vec<_>, LoweringError>>()?
            };
            operations.byte_lengths = inherited_lengths.clone();
            let target = plan
                .states
                .iter()
                .position(|state| state.state == edge.target_state)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph target disappeared during emission",
                ))?;
            let stage = case_edge || condition.is_some()
                    && ((current_rank.is_some() && ranking::has_rank(plan, &plan.states[target])) || edge.transfers.iter().any(|transfer| matches!(
                        transfer.source, checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice { .. }
                    )) || edge.scalar_arguments.iter().any(|argument| {
                        matches!(
                            argument.source,
                            checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
                        )
                    }));
            let operation_start = operations.len();
            let mut arguments = Vec::new();
            let mut structural_arguments = Vec::new();
            let target_state = &plan.states[target];
            for argument_position in
                0..target_state.structural_parameters.len() + target_state.scalar_parameters.len()
            {
                if let Some((target_parameter, transfer)) = target_state
                    .structural_parameters
                    .iter()
                    .zip(&edge.transfers)
                    .find(|(parameter, _)| parameter.position as usize == argument_position)
                {
                    if target_parameter.is_self {
                        let checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                            index,
                        } = transfer.source
                        else {
                            return unsupported("Unit graph receiver cannot be rebound");
                        };
                        if state_parameters
                            .get(index as usize)
                            .map(|parameter| parameter.place)
                            != parameters
                                .iter()
                                .find(|parameter| parameter.is_self)
                                .map(|parameter| parameter.place)
                        {
                            return unsupported(
                                "Unit graph receiver lost original invocation place",
                            );
                        }
                        continue;
                    }
                    let place = match transfer.source {
                            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => case_emission::result(state, binding_ordinal, &operations)?.place,
                            checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                                state_parameters.get(index as usize).ok_or(
                                    LoweringError::Unsupported("Unit graph transfer source descriptor disappeared"),
                                )?.place
                            }
                            checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice { parameter_index, expression } => {
                                let destination = place_id(allocate_dense(&mut catalogs.next_place)?);
                                let source = state_parameters.get(parameter_index as usize).ok_or(
                                    LoweringError::Unsupported("Unit graph subslice source descriptor disappeared"),
                                )?;
                                structural_places.push(subslices::emit(
                                    checked, state, edge.statement_ordinal, target_parameter.position, expression,
                                    source, destination, &bindings, &values, &mut next_value, &mut operations,
                                )?);
                                destination
                            }
                        };
                    structural_arguments.push(StructuralArgument {
                        place,
                        path: Vec::new(),
                        access: match target_parameter.access {
                            checked_trees::CheckedStructuralAccess::Owned => {
                                StructuralAccess::Owned
                            }
                            checked_trees::CheckedStructuralAccess::MutableBorrow => {
                                StructuralAccess::MutableBorrow
                            }
                            _ => StructuralAccess::SharedBorrow,
                        },
                    });
                    continue;
                }
                if let Some((scalar_position, _)) = target_state
                    .scalar_parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.source_position as usize == argument_position)
                    && let Some((_, payload)) = payload_values
                        .iter()
                        .find(|(position, _)| *position as usize == scalar_position)
                {
                    arguments.push(payload.id);
                    continue;
                }
                let transfer = edge
                    .scalar_arguments
                    .iter()
                    .find(|transfer| transfer.argument_ordinal as usize == argument_position)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph successor argument position missing",
                    ))?;
                let expression = match transfer.source {
                    checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter {
                        index,
                    } => bindings.expression(&CheckedScalarExpression::Parameter {
                        position: index as usize,
                        primitive_type: transfer.primitive_type,
                    })?,
                    checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression => {
                        bindings.expression_at(
                            checked,
                            state.state,
                            edge.statement_ordinal,
                            CheckedScalarExpressionRole::TransitionArgument {
                                argument_ordinal: transfer.argument_ordinal,
                            },
                        )?
                    }
                };
                if expression.scalar_type() != terminal_scalar_type(transfer.primitive_type)?
                    || direct_expression_contains_short_circuit(&expression)
                {
                    return unsupported("Unit graph successor needs a matching branch-free value");
                }
                validate_direct_parameter_types(
                    &expression,
                    &values
                        .iter()
                        .map(|value| value.scalar_type)
                        .collect::<Vec<_>>(),
                )?;
                arguments.push(emit_direct_expression(
                    &expression,
                    &values,
                    &mut next_value,
                    &mut operations,
                ));
            }
            let arriving_rank = if current_rank.is_some() {
                if let Some(position) = ranking::scalar_parameter_position(plan, target_state) {
                    arguments.get(position).copied()
                } else {
                    ranking::byte_argument_position(plan, target_state).map(|parameter_position| {
                        crate::operation_emission::emit_byte_length(
                            structural_arguments[parameter_position].place,
                            &mut next_value,
                            &mut operations,
                        )
                    })
                }
            } else {
                None
            };
            let target = state_ids[target];
            if stage {
                let staged = block_id(allocate_dense(&mut next_block)?);
                let backedge = edge_id(allocate_dense(&mut next_edge)?);
                let selection_edge = edge_id(allocate_dense(&mut next_edge)?);
                if let Some(rank) = current_rank {
                    block_ranks.insert(staged, rank);
                    rank_edges.insert(
                        selection_edge,
                        (
                            rank,
                            terminal_psi::TerminalNaturalRankComparison::Preserving,
                        ),
                    );
                    if let Some(after) = arriving_rank {
                        rank_edges.insert(
                            backedge,
                            (after, terminal_psi::TerminalNaturalRankComparison::Strict),
                        );
                    }
                }
                edge_blocks.push(Block {
                    id: staged,
                    parameters: payload_values.iter().map(|(_, value)| *value).collect(),
                    structural_parameters: Vec::new(),
                    operations: operations[operation_start..].to_vec(),
                    terminator: Terminator::Jump {
                        edge: backedge,
                        target,
                        arguments,
                        structural_arguments,
                        trivial_affine_discards,
                        residual_affine_discards: Vec::new(),
                    },
                });
                Ok(SuccessorEdge {
                    edge: selection_edge,
                    target: staged,
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                })
            } else {
                let successor_edge = edge_id(allocate_dense(&mut next_edge)?);
                if let Some(after) = arriving_rank {
                    rank_edges.insert(
                        successor_edge,
                        (after, terminal_psi::TerminalNaturalRankComparison::Strict),
                    );
                }
                Ok(SuccessorEdge {
                    edge: successor_edge,
                    target,
                    arguments,
                    structural_arguments,
                    trivial_affine_discards,
                })
            }
        };
        let mut terminator = match &state.terminator {
            CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result } => {
                let returned_claims = match result.source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => case_emission::result(state, binding_ordinal, &operations)?
                        .claims
                        .iter()
                        .map(|binding| binding.claim)
                        .collect::<Vec<_>>(),
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index,
                    } => state
                        .entry_claims
                        .iter()
                        .filter(|claim| claim.parameter_index == parameter_index)
                        .map(|claim| lookup_claim_id(&claims.source_claims, claim.claim_identity))
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => return unsupported("structural return has no whole claim source"),
                };
                if content_entry_claims.iter().any(|entry| {
                    returned_claims.contains(&entry.claim)
                        && !content_identity_reshuffles.iter().any(|identity| {
                            identity.claim == entry.claim
                                && identity.input == entry.input
                                && identity.projections == entry.projections
                        })
                }) {
                    return unsupported(
                        "structural return lost its checked content identity guarantee",
                    );
                }
                let source = match result.source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => case_emission::result(state, binding_ordinal, &operations)?.place,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index,
                    } => {
                        state_parameters
                            .get(parameter_index as usize)
                            .ok_or(LoweringError::Unsupported(
                                "structural graph return parameter missing",
                            ))?
                            .place
                    }
                    _ => return unsupported("structural graph return needs a whole owned value"),
                };
                Terminator::ReturnStructural {
                    edge: edge_id(allocate_dense(&mut next_edge)?),
                    source: evaluation.current_structural_place(source),
                    returned_claims,
                    trivial_affine_discards: Vec::new(),
                }
            }
            CheckedComposedUnitControlTerminatorPlan::ReturnCase { .. } => {
                Terminator::ReturnStructural {
                    edge: edge_id(allocate_dense(&mut next_edge)?),
                    source: returned_case.ok_or(LoweringError::Unsupported(
                        "case return construction missing",
                    ))?,
                    returned_claims: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                }
            }
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit => {
                let source = checked
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == plan.machine)
                    .and_then(|machine| {
                        checked
                            .machine_states(machine)
                            .iter()
                            .find(|source| source.symbol == state.state)
                    })
                    .ok_or(LoweringError::Unsupported(
                        "Unit return source state missing",
                    ))?;
                let discards = edges::return_discards(checked, plan.machine, source, state)?;
                Terminator::ReturnUnit {
                    edge: edge_id(allocate_dense(&mut next_edge)?),
                    trivial_affine_discards: discards
                        .into_iter()
                        .map(|index| state_parameters[index].place)
                        .collect(),
                }
            }
            CheckedComposedUnitControlTerminatorPlan::Jump { successor: edge } => {
                let edge = successor(edge, &[], false)?;
                Terminator::Jump {
                    edge: edge.edge,
                    target: edge.target,
                    arguments: edge.arguments,
                    structural_arguments: edge.structural_arguments,
                    trivial_affine_discards: edge.trivial_affine_discards,
                    residual_affine_discards: Vec::new(),
                }
            }
            CheckedComposedUnitControlTerminatorPlan::Conditional {
                when_true,
                when_false,
                ..
            } => Terminator::Conditional {
                condition: condition.ok_or(LoweringError::Unsupported(
                    "Unit graph conditional lost its guard",
                ))?,
                when_true: successor(when_true, &[], false)?,
                when_false: successor(when_false, &[], false)?,
            },
            CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. } => {
                let prepared = prepared_cases.as_ref().ok_or(LoweringError::Unsupported(
                    "Unit graph case terminator lost its prepared payloads",
                ))?;
                let cases = prepared
                    .cases
                    .iter()
                    .map(|case| {
                        let edge = successor(case.successor, &case.values, true)?;
                        Ok(StructuralCaseSuccessorEdge {
                            edge: edge.edge,
                            target: edge.target,
                            case: case.identity,
                            payload_fields: case.fields.clone(),
                            trivial_affine_discards: vec![prepared.source],
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                Terminator::StructuralCase {
                    source: prepared.source,
                    cases,
                }
            }
        };
        // Producing a value does not dispose of the remaining entry owners.
        // Reuse the source exit-custody join for structural and Unit returns;
        // the returned owner itself is not a discard.
        if let Terminator::ReturnStructural {
            trivial_affine_discards,
            ..
        } = &mut terminator
        {
            let source = checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == plan.machine)
                .and_then(|machine| {
                    checked
                        .machine_states(machine)
                        .iter()
                        .find(|source| source.symbol == state.state)
                })
                .ok_or(LoweringError::Unsupported(
                    "structural return source state missing",
                ))?;
            *trivial_affine_discards =
                edges::return_discards(checked, plan.machine, source, state)?
                    .into_iter()
                    .map(|index| evaluation.current_structural_place(state_parameters[index].place))
                    .collect();
        }
        if !evaluation.selection_cleanups.is_empty() {
            let discards = match &mut terminator {
                Terminator::ReturnStructural {
                    trivial_affine_discards,
                    ..
                }
                | Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } => trivial_affine_discards,
                _ => {
                    return unsupported(
                        "owned selection residuals crossing authored states require retained cleanup transfer correspondence",
                    );
                }
            };
            let mut local_discards = Vec::new();
            for operation in state.operations.iter().rev() {
                if let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::StructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                } = operation
                {
                    local_discards.push((
                        case_emission::result(state, result.binding_ordinal, &operations)?.place,
                        *discard_result_on_return,
                    ));
                }
            }
            local_discards.extend(discards.iter().map(|place| (*place, true)));
            *discards = evaluation.selection_return_discards(local_discards)?;
        }
        if let Some(rank) = current_rank {
            // Completed evaluation blocks stay inside this authored state.
            // Their private edges preserve its incoming rank; only the state
            // successor constructed above claims an authored strict decrease.
            rank_edges.extend(evaluation.blocks.iter().flat_map(|block| {
                block.terminator.edges().map(|edge| {
                    (
                        edge,
                        (
                            rank,
                            terminal_psi::TerminalNaturalRankComparison::Preserving,
                        ),
                    )
                })
            }));
        }
        evaluation.remap_transported_call_operands(&mut operations);
        evaluation.blocks.push(Block {
            id: evaluation.current,
            parameters: evaluation.parameters,
            structural_parameters: evaluation.block_structural_parameters,
            operations: operations[evaluation.operation_start
                ..if condition.is_some() || prepared_cases.is_some() {
                    body_end
                } else {
                    operations.len()
                }]
                .to_vec(),
            terminator,
        });
        if position != 0 || entry_reentered {
            // Argument evaluation can split the body; bindings belong to its source root.
            let root = evaluation
                .blocks
                .iter_mut()
                .find(|block| block.id == state_ids[position])
                .ok_or(LoweringError::Unsupported(
                    "Unit graph state root disappeared during evaluation",
                ))?;
            root.structural_parameters = std::mem::take(&mut state_views[position])
                .into_iter()
                .filter(|parameter| !parameter.is_self)
                .collect();
        }
        if let Some(rank) = current_rank {
            block_ranks.extend(evaluation.blocks.iter().map(|block| (block.id, rank)));
        }
        blocks.extend(evaluation.blocks);
        blocks.extend(edge_blocks);
        occurrences.extend(operations.source_calls);
        catalogs.next_value = next_value;
        catalogs.next_block = next_block;
        catalogs.next_edge = next_edge;
        catalogs.next_operation = operations.next_identity;
    }
    blocks.sort_by_key(|block| block.id);
    structural_places.extend(
        blocks
            .iter()
            .flat_map(|block| {
                crate::scalar_computations::arrays::declarations(&block.operations).chain(
                    crate::scalar_computations::cases::declarations(&block.operations),
                )
            })
            .filter(|place| {
                !catalogs
                    .result_places
                    .iter()
                    .chain(&catalogs.temporary_places)
                    .any(|existing| existing.id == place.id)
            }),
    );
    structural_places.append(&mut catalogs.temporary_places);
    structural_places.extend(catalogs.result_places.drain(result_places_start..));
    structural_places.sort_by_key(|place| place.id);
    let attachment = plan
        .attachment_type_identity
        .as_ref()
        .map(|identity| lookup_type_id(&catalogs.type_ids, identity))
        .transpose()?;
    if let Some(attachment) = attachment {
        let declaration = catalogs
            .structural_types
            .iter()
            .find(|declaration| declaration.id == attachment)
            .ok_or(LoweringError::Unsupported(
                "Unit graph attachment declaration is absent",
            ))?;
        let boundaries = catalogs
            .lowered_boundaries
            .iter()
            .map(|boundary| (boundary.source, boundary.id))
            .collect::<Vec<_>>();
        structural_places.extend(
            crate::attached_unit::provider_attachments::lower_provider_attachment_places(
                attachment,
                declaration,
                &plan.provider_attachment_requirements,
                &boundaries,
                &mut catalogs.next_place,
            )?,
        );
    }
    let mut machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: crate::attached_unit::lower_declared_service_reach(
            checked,
            plan.machine,
            &catalogs.service_ids,
        )?,
        id: terminal_machine,
        attachment,
        parameters: scalar_parameters,
        structural_places,
        structural_parameters: parameters,
        entry_claims: claims.entry_claims,
        ranked_scc: None,
        result: machine_result,
        published_service_ceiling: lower_installation_machine_service_ceiling(
            checked,
            plan.machine,
            plan.contract_service_reach,
            plan.service_reach,
            &catalogs.service_ids,
        )?,
        content_entry_claims,
        content_identity_reshuffles,
        content_partition_compositions: Vec::new(),
        entry: invocation_entry.unwrap_or(state_ids[0]),
        blocks,
        contract: MachineContract {
            id: contract_id(terminal_machine.get()),
            requires: Vec::new(),
            ensures: Vec::new(),
            crash_routes: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    machine.contract.crash_routes =
        lower_checked_crash_route_buckets(&catalogs.root_crash_routes, &machine.parameters)?;
    ranking::retain(&mut machine, &block_ranks, &rank_edges)?;
    Ok((machine, occurrences))
}

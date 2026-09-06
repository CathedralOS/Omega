//! Three-block Terminal emission after exact admission and catalog projection.

use super::*;

pub(super) fn emit_composed_unit_control(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: admission::AdmittedComposedUnit<'_>,
    mut catalogs: catalogs::ComposedCatalogs,
) -> Result<LoweredPsi, LoweringError> {
    let entry = admitted.entry;
    let entry_parameters = entry
        .scalar_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            Ok(ValueDeclaration {
                id: value_id(dense_identity(index)?),
                scalar_type: terminal_scalar_type(parameter.primitive_type)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut scalar_values = entry_parameters.clone();
    let mut scalar_value_types = entry_parameters
        .iter()
        .map(|parameter| parameter.scalar_type)
        .collect::<Vec<_>>();
    let mut next_place = catalogs.next_place;
    let structural_parameters = lower_unit_parameters(
        &entry.structural_parameters,
        &catalogs.type_ids,
        &[],
        &mut next_place,
    )?;
    let claims = super::super::claims::lower_unit_entry_claims(
        plan.machine,
        entry.state,
        &entry.entry_claims,
        &structural_parameters,
    )?;
    let mut claim_bindings = claims.source_claims;
    if let custody::ComposedCustody::WholeRootLinear {
        entry_claim,
        leaf_claims,
    } = admitted.custody
    {
        let claim = lookup_claim_id(&claim_bindings, entry_claim)?;
        for alias in leaf_claims {
            if claim_bindings.iter().any(|(source, _)| *source == alias) {
                return unsupported("composed Unit leaf claim alias is duplicated");
            }
            claim_bindings.push((alias, claim));
        }
    }
    let content_entry_claims = content_conservation::lower_whole_content_entry_claims(
        checked,
        &entry.structural_parameters,
        &structural_parameters,
        &entry.entry_claims,
        &claim_bindings,
    )?;
    let CheckedComposedUnitControlTerminatorPlan::Conditional { guard, .. } = &entry.terminator
    else {
        unreachable!("admission retained one conditional entry")
    };
    let state_ids = [block_id(1), block_id(2), block_id(3)];
    let mut next_block = 4_u64;
    let mut next_value = u64::try_from(entry_parameters.len())
        .map_err(|_| LoweringError::Unsupported("composed Unit scalar arity exceeds u64"))?
        + 1;
    let mut next_edge = 1_u64;
    let mut entry_operations = OperationBuffer::new(0);
    for (binding_index, binding) in entry.bindings.iter().enumerate() {
        let ordinal = u32::try_from(binding_index)
            .map_err(|_| LoweringError::Unsupported("composed Unit binding index exceeds u32"))?;
        if binding.statement_ordinal != ordinal
            || binding.destination != checked_trees::CheckedScalarBindingDestination::Immutable
            || binding.value != CheckedScalarBindingValue::Expression
        {
            return unsupported("composed Unit binding order drifted after admission");
        }
        let expected_type = terminal_scalar_type(binding.primitive_type)?;
        let initializer = lower_checked_scalar_expression_at(
            checked,
            entry.state,
            ordinal,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: ordinal,
            },
        )?;
        validate_direct_parameter_types(&initializer, &scalar_value_types)?;
        if initializer.scalar_type() != expected_type {
            return unsupported("composed Unit initializer type drifted from its checked binding");
        }
        let id = emit_direct_expression(
            &initializer,
            &scalar_values,
            &mut next_value,
            &mut entry_operations,
        );
        scalar_values.push(ValueDeclaration {
            id,
            scalar_type: expected_type,
        });
        scalar_value_types.push(expected_type);
    }
    let guard = lower_checked_scalar_expression(guard)?;
    validate_direct_parameter_types(&guard, &scalar_value_types)?;
    let guard = emit_direct_expression(
        &guard,
        &scalar_values,
        &mut next_value,
        &mut entry_operations,
    );
    let mut next_operation = entry_operations.next_identity;
    let mut blocks = vec![Block {
        id: state_ids[0],
        parameters: Vec::new(),
        operations: entry_operations.operations,
        terminator: Terminator::Conditional {
            condition: guard,
            when_true: empty_successor(state_ids[1], &mut next_edge)?,
            when_false: empty_successor(state_ids[2], &mut next_edge)?,
        },
    }];
    let mut source_call_occurrences = Vec::new();
    for (state, block) in admitted.leaves.into_iter().zip(&state_ids[1..]) {
        let (lowered_block, mut occurrences) = match &state.operations[0] {
            CheckedUnitEffectOperationPlan::BoundaryCall { .. } => emit_boundary_leaf(
                checked,
                plan.machine,
                state,
                *block,
                &mut catalogs,
                &structural_parameters,
                &claim_bindings,
                &[],
                &mut next_value,
                &mut next_block,
                &mut next_operation,
                &mut next_edge,
            )?,
            CheckedUnitEffectOperationPlan::CallUnit { .. } => {
                let (block, occurrences) = internal_calls::emission::emit_leaf(
                    state,
                    *block,
                    &catalogs.internal_targets,
                    &mut next_operation,
                    &mut next_edge,
                )?;
                (vec![block], occurrences)
            }
            _ => unreachable!("admission retained one exact leaf call"),
        };
        blocks.extend(lowered_block);
        source_call_occurrences.append(&mut occurrences);
    }
    let attachment = lookup_type_id(&catalogs.type_ids, &plan.attachment_type_identity)?;
    let attachment_declaration = catalogs
        .structural_types
        .iter()
        .find(|declaration| declaration.id == attachment)
        .expect("composed attachment declaration was selected");
    let provider_boundaries = catalogs
        .lowered_boundaries
        .iter()
        .map(|boundary| (boundary.source, boundary.id))
        .collect::<Vec<_>>();
    let structural_places = super::super::provider_attachments::lower_provider_attachment_places(
        attachment,
        attachment_declaration,
        &plan.provider_attachment_requirements,
        &provider_boundaries,
        &mut next_place,
    )?;
    let machine = TerminalMachine {
        id: machine_id(1),
        attachment: Some(attachment),
        parameters: entry_parameters,
        structural_parameters: structural_parameters.clone(),
        ranked_scc: None,
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
            .chain(structural_places)
            .collect(),
        entry_claims: claims.entry_claims,
        published_service_ceiling: lower_installation_machine_service_ceiling(
            checked,
            plan.machine,
            plan.contract_service_reach,
            plan.service_reach,
            &catalogs.service_ids,
        )?,
        content_entry_claims,
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: state_ids[0],
        blocks,
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let mut machines = vec![machine];
    machines.extend(internal_calls::emission::emit_targets(
        checked,
        &catalogs.internal_targets,
        &catalogs.type_ids,
        &catalogs.service_ids,
        &mut next_operation,
        &mut next_block,
        &mut next_edge,
    )?);
    finish_module(machines, catalogs, source_call_occurrences)
}

pub(crate) fn emit_boundary_leaf(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    block: BlockId,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    scalar_parameters: &[ValueDeclaration],
    next_value: &mut u64,
    next_block: &mut u64,
    next_operation: &mut u64,
    next_edge: &mut u64,
) -> Result<(Vec<Block>, Vec<LoweredSourceCallOccurrence>), LoweringError> {
    let mut operations = OperationBuffer::new(*next_operation - 1);
    let mut evaluation = super::super::argument_evaluation::Evaluation {
        entry: block,
        current: block,
        parameters: scalar_parameters.to_vec(),
        operation_start: 0,
        blocks: Vec::new(),
    };
    let mut values = scalar_parameters.to_vec();
    for operation in &state.operations {
        let mut calls = catalogs.scalar_calls.emission_context();
        let arguments = evaluation.arguments(
            checked,
            machine,
            state.state,
            operation,
            &mut values,
            next_value,
            next_block,
            next_edge,
            &mut operations,
            &mut calls,
        )?;
        catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
        emit_boundary_call_operation(
            state,
            operation,
            &catalogs.lowered_boundaries,
            &catalogs.type_ids,
            &catalogs.structural_types,
            parameters,
            claim_bindings,
            arguments.as_deref(),
            &mut operations,
        )?;
    }
    *next_operation = operations.next_identity;
    evaluation.blocks.push(Block {
        id: evaluation.current,
        parameters: evaluation.parameters,
        operations: operations[evaluation.operation_start..].to_vec(),
        terminator: Terminator::ReturnUnit {
            edge: edge_id(allocate_dense(next_edge)?),
            trivial_affine_discards: Vec::new(),
        },
    });
    Ok((evaluation.blocks, operations.source_calls))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_boundary_call_operation(
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    boundaries: &[catalogs::LoweredComposedBoundary],
    type_ids: &[(String, StructuralTypeId)],
    structural_types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    scalar_values: Option<&[ValueDeclaration]>,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        source_site,
        target_machine,
        scalar_arguments,
        structural_arguments,
        completion_receipts,
        ..
    } = operation
    else {
        unreachable!("admission retained one boundary call")
    };
    let target = boundaries
        .iter()
        .find(|candidate| candidate.source == *target_machine)
        .ok_or(LoweringError::Unsupported(
            "composed Unit boundary target is absent from its exact catalog",
        ))?;
    if scalar_arguments.len() != target.scalar_parameters.len() {
        return unsupported("composed Unit boundary scalar arity drifted");
    }
    let expected_claim_arguments = structural_arguments
        .iter()
        .enumerate()
        .flat_map(|(argument_index, argument)| {
            state
                .entry_claims
                .iter()
                .filter(move |claim| {
                    Some(claim.parameter_index) == argument.source_parameter_index()
                        && (argument.path.is_empty() || claim.path == argument.path)
                })
                .map(move |_| {
                    u32::try_from(argument_index).map_err(|_| {
                        LoweringError::Unsupported(
                            "composed Unit boundary argument index exceeds u32",
                        )
                    })
                })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    validate_transfer_shape(
        structural_arguments,
        completion_receipts,
        parameters,
        &[],
        &[],
        &target.checked_structural_parameters,
        type_ids,
        structural_types,
        &expected_claim_arguments,
    )?;
    let arguments = super::super::argument_evaluation::validated_values(
        scalar_values,
        &target.scalar_parameters,
    )?
    .into_iter()
    .map(|value| value.id)
    .collect();
    let call_id = operations.allocate();
    operations.record_source_call(
        SourceCallCoordinate {
            state: state.state,
            statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                LoweringError::Unsupported("composed Unit statement coordinate exceeds usize")
            })?,
            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("composed Unit call coordinate exceeds usize")
            })?,
        },
        *source_site,
        call_id,
        *target_machine,
    )?;
    operations.push(Operation {
        id: call_id,
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: target.id,
            arguments,
            structural_arguments: lower_structural_arguments(
                structural_arguments,
                parameters,
                &[],
                &[],
                &[],
            )?,
            completion_receipts: completion_receipts
                .iter()
                .map(|receipt| {
                    Ok(CompletionReceipt {
                        claim: lookup_claim_id(claim_bindings, receipt.claim_identity)?,
                        argument_index: receipt.argument_index,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
        },
    });
    Ok(())
}

fn empty_successor(target: BlockId, next_edge: &mut u64) -> Result<SuccessorEdge, LoweringError> {
    Ok(SuccessorEdge {
        edge: edge_id(allocate_dense(next_edge)?),
        target,
        arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    })
}

pub(super) fn finish_module(
    mut machines: Vec<TerminalMachine>,
    catalogs: catalogs::ComposedCatalogs,
    source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
) -> Result<LoweredPsi, LoweringError> {
    for machine in &mut machines {
        machine.blocks.sort_by_key(|block| block.id);
    }
    let root = machines.first_mut().ok_or(LoweringError::Unsupported(
        "composed Unit emission produced no entry machine",
    ))?;
    root.contract.crash_routes =
        lower_checked_crash_route_buckets(&catalogs.root_crash_routes, &root.parameters)?;
    let entry = machines
        .first()
        .map(|machine| machine.id)
        .ok_or(LoweringError::Unsupported(
            "composed Unit emission produced no entry machine",
        ))?;
    let mut lowered = LoweredPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry,
            structural_types: catalogs.structural_types,
            structural_domains: Vec::new(),
            services: catalogs.services,
            root_service_reach: catalogs.root_service_reach,
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: catalogs.boundary_machines,
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines,
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences: Vec::new(),
    };
    catalogs.scalar_calls.append_to(&mut lowered)?;
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

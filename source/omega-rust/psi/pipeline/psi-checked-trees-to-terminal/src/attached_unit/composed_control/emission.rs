//! Three-block Terminal emission after exact admission and catalog projection.

use super::*;

pub(super) fn emit_composed_unit_control(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: admission::AdmittedComposedUnit<'_>,
    catalogs: catalogs::ComposedCatalogs,
) -> Result<LoweredTerminalPsi, LoweringError> {
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
    let mut next_value = u64::try_from(entry_parameters.len())
        .map_err(|_| LoweringError::Unsupported("composed Unit scalar arity exceeds u64"))?
        + 1;
    let mut next_edge = 1_u64;
    let mut entry_operations = OperationBuffer::new(0);
    for (binding_index, binding) in entry.bindings.iter().enumerate() {
        let ordinal = u32::try_from(binding_index)
            .map_err(|_| LoweringError::Unsupported("composed Unit binding index exceeds u32"))?;
        if binding.statement_ordinal != ordinal
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
                state,
                *block,
                &catalogs.lowered_boundaries,
                &catalogs.type_ids,
                &catalogs.structural_types,
                &structural_parameters,
                &claim_bindings,
                &mut next_value,
                &mut next_operation,
                &mut next_edge,
            )?,
            CheckedUnitEffectOperationPlan::CallUnit { .. } => internal_calls::emission::emit_leaf(
                state,
                *block,
                &catalogs.internal_targets,
                &mut next_operation,
                &mut next_edge,
            )?,
            _ => unreachable!("admission retained one exact leaf call"),
        };
        blocks.push(lowered_block);
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
    let mut next_block = 4_u64;
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
    state: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
    block: BlockId,
    boundaries: &[catalogs::LoweredComposedBoundary],
    type_ids: &[(String, StructuralTypeId)],
    structural_types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    next_value: &mut u64,
    next_operation: &mut u64,
    next_edge: &mut u64,
) -> Result<(Block, Vec<LoweredSourceCallOccurrence>), LoweringError> {
    let mut operations = OperationBuffer::new(*next_operation - 1);
    emit_boundary_call_operation(
        state,
        &state.operations[0],
        boundaries,
        type_ids,
        structural_types,
        parameters,
        claim_bindings,
        next_value,
        &mut operations,
    )?;
    *next_operation = operations.next_identity;
    let OperationBuffer {
        operations,
        source_calls,
        ..
    } = operations;
    Ok((
        Block {
            id: block,
            parameters: Vec::new(),
            operations,
            terminator: Terminator::ReturnUnit {
                edge: edge_id(allocate_dense(next_edge)?),
                trivial_affine_discards: Vec::new(),
            },
        },
        source_calls,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_boundary_call_operation(
    state: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    boundaries: &[catalogs::LoweredComposedBoundary],
    type_ids: &[(String, StructuralTypeId)],
    structural_types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    next_value: &mut u64,
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
                    claim.parameter_index == argument.source_parameter_index
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
        &target.checked_structural_parameters,
        type_ids,
        structural_types,
        &expected_claim_arguments,
    )?;
    let arguments = scalar_arguments
        .iter()
        .zip(&target.scalar_parameters)
        .map(|(argument, target_type)| {
            let argument = lower_checked_scalar_expression(argument)?;
            if argument.scalar_type() != *target_type {
                return unsupported("composed Unit boundary scalar type drifted");
            }
            validate_direct_parameter_types(&argument, &[])?;
            Ok(emit_direct_expression(
                &argument,
                &[],
                next_value,
                operations,
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
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
    machines: Vec<TerminalMachine>,
    catalogs: catalogs::ComposedCatalogs,
    source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let entry = machines
        .first()
        .map(|machine| machine.id)
        .ok_or(LoweringError::Unsupported(
            "composed Unit emission produced no entry machine",
        ))?;
    let mut lowered = LoweredTerminalPsi {
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
            quotient_correspondences: Vec::new(),
            machines,
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences: Vec::new(),
    };
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

//! One direct named-dynamic scalar result followed by checked Unit control.

use super::*;

pub(super) fn lower(
    checked: &CheckedTrees,
    plan: &CheckedDirectDynamicScalarCallPlan,
    continuation: &psi_checked_trees::CheckedDirectDynamicUnitContinuationPlan,
    caller: DirectCallerShape,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if plan.caller_structural_scalar_field_store.is_some() {
        return unsupported(
            "direct dynamic result control cannot also retain a caller field store",
        );
    }
    let catalogs =
        crate::attached_unit::lower_direct_dynamic_control_catalogs(checked, plan, continuation)?;
    if !catalogs.internal_targets.is_empty() || catalogs.next_place != 1 {
        return unsupported(
            "direct dynamic continuation requires scalar-only boundary effect leaves",
        );
    }
    let caller_attachment = lookup_type_id(&catalogs.type_ids, &caller.attachment_type_identity)?;
    let caller_self = StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: true,
        structural_type: caller_attachment,
        multiplicity: terminal_structural_multiplicity(plan.caller_multiplicity),
        access: match plan.caller_parameter_access {
            CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
            CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
            _ => return unsupported("direct dynamic caller requires a borrowed self parameter"),
        },
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let source = validate_and_lower_source(
        &caller_self,
        plan,
        &catalogs.structural_types,
        &catalogs.type_ids,
    )?;

    let caller_machine = machine_id(1);
    let realization_machine = machine_id(2);
    let call_operation = operation_id(1);
    let call_result_value = value_id(1);
    let call_result_type = terminal_scalar_type(plan.result.primitive_type)?;
    let call_result = ValueDeclaration {
        id: call_result_value,
        scalar_type: call_result_type,
    };
    let callable_result = terminal_callable_result(plan.result.primitive_type)?;
    let callable_identity =
        evidence_lowering::checked_evidence_machine_identity(checked, plan.realization_machine)?;
    if callable_identity != plan.realization_identity {
        return unsupported("direct dynamic realization callable identity drifted");
    }
    let (application, selected_row) = lower_exact_application(
        checked,
        plan,
        caller_machine,
        realization_machine,
        callable_result,
        &callable_identity,
    )?;
    let selection = TerminalDynamicConformanceSelection {
        owner: caller_machine,
        ordinal: 0,
        source: source.clone(),
        conformance_application_report_fingerprint: application.report_fingerprint,
        conformance_application_commitment: application.commitment,
    };
    let dispatch = TerminalDirectDynamicDispatch {
        owner: caller_machine,
        operation: call_operation,
        selection_ordinal: 0,
        declaring_trait_identity: selected_row.declaring_trait_identity.clone(),
        public_requirement_identity: selected_row.public_requirement_identity.clone(),
        requirement_identity: selected_row.requirement_identity.clone(),
        realization_identity: selected_row.realization_identity.clone(),
        realization_callable_identity: callable_identity,
        realization: realization_machine,
    };

    let mut caller_operations = vec![Operation {
        id: call_operation,
        result: OperationResult::Scalar(call_result),
        kind: OperationKind::CallStructuralScalar {
            callee: realization_machine,
            structural_arguments: vec![source],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }];
    let guard = lower_checked_scalar_expression(&continuation.guard)?;
    validate_direct_parameter_types(&guard, &[call_result_type])?;
    let mut next_value = 2;
    let mut guard_operations = OperationBuffer::new(call_operation.get());
    let condition = emit_direct_expression(
        &guard,
        std::slice::from_ref(&call_result),
        &mut next_value,
        &mut guard_operations,
    );
    let mut next_operation = guard_operations.next_identity;
    caller_operations.extend(guard_operations.operations);

    let caller_block = block_id(1);
    let leaf_blocks = [block_id(2), block_id(3)];
    let realization_block = block_id(4);
    let mut next_edge = 1_u64;
    let mut caller_blocks = vec![Block {
        id: caller_block,
        parameters: Vec::new(),
        operations: caller_operations,
        terminator: Terminator::Conditional {
            condition,
            when_true: empty_successor(leaf_blocks[0], &mut next_edge)?,
            when_false: empty_successor(leaf_blocks[1], &mut next_edge)?,
        },
    }];
    let mut source_call_occurrences = vec![LoweredSourceCallOccurrence {
        source_site: None,
        source_state: plan.caller_state,
        statement_index: usize::try_from(plan.coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported("direct dynamic statement coordinate exceeds usize")
        })?,
        call_ordinal: usize::try_from(plan.coordinate.call_ordinal)
            .map_err(|_| LoweringError::Unsupported("direct dynamic call ordinal exceeds usize"))?,
        terminal_operation: call_operation,
        source_target: plan.requirement,
    }];
    for (state, block) in continuation.leaves.iter().zip(leaf_blocks) {
        let (leaf, mut occurrences) = crate::attached_unit::emit_direct_dynamic_boundary_leaf(
            state,
            block,
            &catalogs.lowered_boundaries,
            &catalogs.type_ids,
            &catalogs.structural_types,
            &[],
            &[],
            &mut next_value,
            &mut next_operation,
            &mut next_edge,
        )?;
        caller_blocks.push(leaf);
        source_call_occurrences.append(&mut occurrences);
    }

    let source_type = lookup_type_id(&catalogs.type_ids, &plan.source_type_identity)?;
    let realization_parameter = StructuralParameterDeclaration {
        place: place_id(2),
        position: 0,
        is_self: true,
        structural_type: source_type,
        multiplicity: terminal_projected_source_multiplicity(plan),
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let realization_machine_result_value = value_id(next_value);
    next_value = next_value.checked_add(1).ok_or(LoweringError::Unsupported(
        "direct dynamic continuation value identity space is exhausted",
    ))?;
    let realization_result_value = value_id(next_value);
    let realization_operation = operation_id(next_operation);
    let realization_operations = lower_realization_operations(
        &plan.realization_return_expression,
        call_result_type,
        &realization_parameter,
        &catalogs.structural_types,
        realization_operation,
        realization_result_value,
    )?;
    let realization_value = realization_operations
        .last()
        .and_then(|operation| operation.result.scalar())
        .map(|result| result.id)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic realization did not emit one scalar result",
        ))?;

    let caller_contract_service_reach = checked
        .facts
        .service_reaches
        .plan_for_machine(plan.caller_machine)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic continuation has no caller service contract",
        ))?;
    let caller_reach = lower_installation_machine_service_ceiling(
        checked,
        plan.caller_machine,
        caller_contract_service_reach,
        plan.caller_service_reach,
        &catalogs.service_ids,
    )?;
    let realization_reach = exact_empty_machine_service_ceiling(
        checked,
        plan.realization_machine,
        plan.checked_call_service_reach,
    )?;
    let attachment = catalogs
        .structural_types
        .iter()
        .find(|declaration| declaration.id == caller_attachment)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic continuation attachment declaration is absent",
        ))?;
    let provider_boundaries = catalogs
        .lowered_boundaries
        .iter()
        .map(|boundary| (boundary.source, boundary.id))
        .collect::<Vec<_>>();
    let mut next_place = 3_u64;
    let provider_places = crate::attached_unit::lower_provider_attachment_places(
        caller_attachment,
        attachment,
        &continuation.provider_attachment_requirements,
        &provider_boundaries,
        &mut next_place,
    )?;
    let caller_structural_places = std::iter::once(StructuralPlaceDeclaration {
        id: caller_self.place,
        kind: StructuralPlaceKind::Parameter {
            position: caller_self.position,
            is_self: caller_self.is_self,
        },
    })
    .chain(provider_places)
    .collect();
    let realization_structural_places = vec![StructuralPlaceDeclaration {
        id: realization_parameter.place,
        kind: StructuralPlaceKind::Parameter {
            position: realization_parameter.position,
            is_self: realization_parameter.is_self,
        },
    }];
    let realization_return_edge = edge_id(allocate_dense(&mut next_edge)?);

    Ok(LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: caller_machine,
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
            closed_conformance_applications: vec![application],
            dynamic_dispatch: TerminalDynamicDispatchCatalog {
                selections: vec![selection],
                direct_dispatches: vec![dispatch],
            },
            quotient_correspondences: Vec::new(),
            machines: vec![
                TerminalMachine {
                    id: caller_machine,
                    attachment: Some(caller_attachment),
                    parameters: Vec::new(),
                    structural_parameters: vec![caller_self],
                    ranked_scc: None,
                    result: TerminalMachineResult::Unit,
                    structural_places: caller_structural_places,
                    entry_claims: Vec::new(),
                    published_service_ceiling: caller_reach,
                    content_entry_claims: Vec::new(),
                    content_identity_reshuffles: Vec::new(),
                    content_partition_compositions: Vec::new(),
                    entry: caller_block,
                    blocks: caller_blocks,
                    contract: empty_terminal_contract(caller_machine.get()),
                },
                TerminalMachine {
                    id: realization_machine,
                    attachment: Some(source_type),
                    parameters: Vec::new(),
                    structural_parameters: vec![realization_parameter],
                    ranked_scc: None,
                    result: TerminalMachineResult::Scalar(ValueDeclaration {
                        id: realization_machine_result_value,
                        scalar_type: call_result_type,
                    }),
                    structural_places: realization_structural_places,
                    entry_claims: Vec::new(),
                    published_service_ceiling: realization_reach,
                    content_entry_claims: Vec::new(),
                    content_identity_reshuffles: Vec::new(),
                    content_partition_compositions: Vec::new(),
                    entry: realization_block,
                    blocks: vec![Block {
                        id: realization_block,
                        parameters: Vec::new(),
                        operations: realization_operations,
                        terminator: Terminator::Return {
                            cleanup_actions: Vec::new(),
                            edge: realization_return_edge,
                            value: realization_value,
                        },
                    }],
                    contract: empty_terminal_contract(realization_machine.get()),
                },
            ],
        },
        proof_bundle: ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        },
        debug_map: None,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences: Vec::new(),
    })
}

fn empty_successor(target: BlockId, next_edge: &mut u64) -> Result<SuccessorEdge, LoweringError> {
    Ok(SuccessorEdge {
        edge: edge_id(allocate_dense(next_edge)?),
        target,
        arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    })
}

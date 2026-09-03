//! One direct named-dynamic scalar result followed by checked Unit control.

use super::*;

pub(super) fn lower(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    continuation: &psi_checked_trees::CheckedDynamicUnitContinuationPlan,
    caller: DynamicCallerShape,
    lane: DynamicLoweringLane<'_>,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if plan.caller_structural_scalar_field_store.is_some() {
        return unsupported(
            "direct dynamic result control cannot also retain a caller field store",
        );
    }
    let stored = match lane {
        DynamicLoweringLane::Stored(stored) => Some(stored),
        _ => None,
    };
    let catalogs =
        crate::attached_unit::lower_dynamic_control_catalogs(checked, plan, continuation, stored)?;
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
    let has_descriptor_store = matches!(lane, DynamicLoweringLane::Stored(_));
    let call_operation = operation_id(if has_descriptor_store { 2 } else { 1 });
    let call_result_value = value_id(1);
    let call_result_type = terminal_scalar_type(plan.result.primitive_type)?;
    let call_result = ValueDeclaration {
        id: call_result_value,
        scalar_type: call_result_type,
    };
    let source_type = lookup_type_id(&catalogs.type_ids, &plan.source_type_identity)?;
    let all_realizations = collect_dynamic_realizations(checked, plan)?;
    let lowered_realizations = retain_realizations_for_lane(&all_realizations, plan, lane)?;
    let selected_realizations = lowered_realizations
        .iter()
        .filter(|candidate| {
            candidate.source_machine == plan.realization_machine
                && candidate.source_state == plan.realization_state
        })
        .collect::<Vec<_>>();
    let [selected_realization] = selected_realizations.as_slice() else {
        return unsupported("direct dynamic selected realization is absent or ambiguous");
    };
    let realization_machine = selected_realization.machine;
    let callable_identity = selected_realization.callable_identity.clone();
    if selected_realization.result != terminal_callable_result(plan.result.primitive_type)?
        || callable_identity != plan.realization_identity
    {
        return unsupported("direct dynamic selected realization callable drifted");
    }
    let (application, selected_row) =
        lower_exact_application(checked, plan, caller_machine, &lowered_realizations)?;
    let initial_application = match lane {
        DynamicLoweringLane::Rebound(initial)
            if initial.fact.conformance != plan.selection.conformance
                || initial.fact.rows != plan.selection.rows =>
        {
            Some(lower_initial_rebound_application(
                checked,
                plan.target_trait,
                initial,
                caller_machine,
            )?)
        }
        _ => None,
    };
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
    let guard_operations = guard_operations.operations;

    let caller_block = block_id(1);
    let leaf_blocks = [block_id(2), block_id(3)];
    let mut next_edge = 1_u64;
    let when_true = empty_successor(leaf_blocks[0], &mut next_edge)?;
    let when_false = empty_successor(leaf_blocks[1], &mut next_edge)?;
    let mut emitted_leaf_blocks = Vec::new();
    let mut leaf_source_call_occurrences = Vec::new();
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
        emitted_leaf_blocks.push(leaf);
        leaf_source_call_occurrences.append(&mut occurrences);
    }

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
    let mut next_place = 2_u64;
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
    let mut next_block = 4_u64;
    let forwarded_helpers = forwarded_helper_chain_ids(
        plan,
        &lowered_realizations,
        &mut next_block,
        &mut next_operation,
        &mut next_value,
        &mut next_edge,
    )?;
    let (mut dynamic_dispatch, call_kind) = lower_dynamic_call_custody(
        lane,
        &caller_self,
        plan,
        &catalogs.structural_types,
        &catalogs.type_ids,
        caller_machine,
        call_operation,
        source,
        initial_application.as_ref(),
        &application,
        &selected_row,
        callable_identity,
        realization_machine,
        forwarded_helpers.first().copied(),
    )?;
    if forwarded_helpers.len() > 1 {
        extend_parameter_forwarding_catalog(&mut dynamic_dispatch, &forwarded_helpers)?;
    }
    let mut caller_operations = Vec::new();
    if has_descriptor_store {
        caller_operations.push(Operation {
            id: operation_id(1),
            result: OperationResult::Unit,
            kind: OperationKind::StoreDynamicDescriptor {
                descriptor_ordinal: 0,
            },
        });
    }
    caller_operations.push(Operation {
        id: call_operation,
        result: OperationResult::Scalar(call_result),
        kind: call_kind,
    });
    caller_operations.extend(guard_operations);
    let mut caller_blocks = vec![Block {
        id: caller_block,
        parameters: Vec::new(),
        operations: caller_operations,
        terminator: Terminator::Conditional {
            condition,
            when_true,
            when_false,
        },
    }];
    caller_blocks.extend(emitted_leaf_blocks);
    let mut source_call_occurrences =
        dynamic_source_call_occurrences_for_chain(plan, call_operation, &forwarded_helpers)?;
    source_call_occurrences.append(&mut leaf_source_call_occurrences);
    let realization_machines = materialize_dynamic_realizations(
        checked,
        plan,
        &lowered_realizations,
        source_type,
        &catalogs.structural_types,
        &mut next_block,
        &mut next_place,
        &mut next_operation,
        &mut next_value,
        &mut next_edge,
    )?;
    let forwarded_helper_machines = materialize_forwarded_helper_chain(
        checked,
        plan,
        &application,
        &selected_row,
        &forwarded_helpers,
    )?;

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
            closed_conformance_applications: {
                let mut applications = vec![application];
                applications.extend(initial_application);
                applications.sort_by(|left, right| {
                    (
                        left.owner,
                        left.declaration_identity.as_str(),
                        left.report_fingerprint,
                    )
                        .cmp(&(
                            right.owner,
                            right.declaration_identity.as_str(),
                            right.report_fingerprint,
                        ))
                });
                applications
            },
            dynamic_dispatch,
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: {
                let mut machines = vec![TerminalMachine {
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
                }];
                machines.extend(realization_machines);
                machines.extend(forwarded_helper_machines);
                machines
            },
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

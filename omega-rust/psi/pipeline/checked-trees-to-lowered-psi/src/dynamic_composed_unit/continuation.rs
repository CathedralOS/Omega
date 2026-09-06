//! One direct named-dynamic scalar result followed by checked Unit control.

use super::*;

pub(super) fn lower(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    continuation: &checked_trees::CheckedDynamicUnitContinuationPlan,
    caller: DynamicCallerShape,
    lane: DynamicLoweringLane<'_>,
) -> Result<crate::machine_dispatch::SourceMappedLowered, LoweringError> {
    if plan.caller_structural_scalar_field_store.is_some() {
        return unsupported(
            "direct dynamic result control cannot also retain a caller field store",
        );
    }
    let stored = match lane {
        DynamicLoweringLane::Stored(stored) => Some(stored),
        _ => None,
    };
    let mut catalogs =
        crate::attached_unit::lower_dynamic_control_catalogs(checked, plan, continuation, stored)?;
    let mut next_place = catalogs.next_place;
    let caller_attachment = lookup_type_id(&catalogs.type_ids, &caller.attachment_type_identity)?;
    let caller_self = StructuralParameterDeclaration {
        place: place_id(allocate_dense(&mut next_place)?),
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
    let mut next_operation = catalogs.next_operation;
    let descriptor_store_operation = if has_descriptor_store {
        Some(operation_id(allocate_dense(&mut next_operation)?))
    } else {
        None
    };
    let call_operation = operation_id(allocate_dense(&mut next_operation)?);
    let mut next_value = catalogs.next_value;
    let call_result_value = value_id(allocate_dense(&mut next_value)?);
    let call_result_type = terminal_scalar_type(plan.result.primitive_type)?;
    let call_result = ValueDeclaration {
        id: call_result_value,
        scalar_type: call_result_type,
    };
    let source_type = lookup_type_id(&catalogs.type_ids, &plan.source_type_identity)?;
    // Shared Unit bodies already contain calls using their selected identities.
    // Dynamic realizations follow them; those existing calls cannot be renamed.
    let first_realization = catalogs
        .shared_units
        .as_ref()
        .map_or(1, |shared| {
            shared
                .semantic_module
                .machines
                .iter()
                .map(|machine| machine.id.get())
                .max()
                .unwrap_or(1)
        })
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "dynamic realization prefix overflowed",
        ))?;
    let all_realizations = collect_dynamic_realizations(checked, plan, first_realization)?;
    let lowered_realizations = retain_realizations_for_lane(&all_realizations, plan, lane)?;
    let realization_prefix = lowered_realizations
        .iter()
        .map(|realization| realization.machine.get())
        .max()
        .unwrap_or(1);
    let forwarded_count = if matches!(
        plan.origin,
        checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { .. }
    ) {
        plan.forwarding_transfers
            .len()
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "dynamic forwarding machine count overflows usize",
            ))?
    } else {
        0
    };
    let scalar_prefix = usize::try_from(realization_prefix)
        .ok()
        .and_then(|count| count.checked_add(forwarded_count))
        .ok_or(LoweringError::Unsupported(
            "dynamic machine prefix exceeds usize",
        ))?;
    if catalogs.shared_units.is_none() {
        catalogs
            .scalar_calls
            .reserve_machine_prefix(scalar_prefix)?;
    }
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
    let mut guard_operations = OperationBuffer::new(call_operation.get());
    let condition = emit_direct_expression(
        &guard,
        std::slice::from_ref(&call_result),
        &mut next_value,
        &mut guard_operations,
    );
    next_operation = guard_operations.next_identity;
    let guard_operations = guard_operations.operations;

    let mut next_block = catalogs.next_block;
    let caller_block = block_id(allocate_dense(&mut next_block)?);
    let leaf_blocks = [
        block_id(allocate_dense(&mut next_block)?),
        block_id(allocate_dense(&mut next_block)?),
    ];
    let mut next_edge = catalogs.next_edge;
    let when_true = empty_successor(leaf_blocks[0], &mut next_edge)?;
    let when_false = empty_successor(leaf_blocks[1], &mut next_edge)?;
    let mut emitted_leaf_blocks = Vec::new();
    let mut leaf_source_call_occurrences = Vec::new();
    for (state, block) in continuation.leaves.iter().zip(leaf_blocks) {
        let (leaf, mut occurrences) = crate::attached_unit::emit_dynamic_control_leaf(
            checked,
            plan.caller_machine,
            state,
            block,
            &mut catalogs,
            &[],
            &[],
            &[],
            &mut next_value,
            &mut next_block,
            &mut next_operation,
            &mut next_edge,
        )?;
        emitted_leaf_blocks.extend(leaf);
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
        descriptor_store_operation,
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
    if let Some(id) = descriptor_store_operation {
        caller_operations.push(Operation {
            id,
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

    let source_machine_ids = catalogs.scalar_calls.machine_ids.clone();
    let mut lowered = catalogs.shared_units.take().unwrap_or_else(|| LoweredPsi {
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
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: Vec::new(),
        },
        proof_bundle: ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        },
        debug_map: None,
        source_call_occurrences: Vec::new(),
        selected_ieee_float_fma_occurrences: Vec::new(),
    });
    if lowered.semantic_module.entry != caller_machine {
        return unsupported("dynamic continuation lost its reserved shared entry");
    }
    let mut contract = empty_terminal_contract(caller_machine.get());
    contract.crash_routes = lower_checked_crash_route_buckets(&catalogs.root_crash_routes, &[])?;
    caller_blocks.sort_by_key(|block| block.id);
    lowered.semantic_module.machines.insert(
        0,
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
            contract,
        },
    );
    lowered
        .semantic_module
        .machines
        .extend(realization_machines);
    lowered
        .semantic_module
        .machines
        .extend(forwarded_helper_machines);
    lowered
        .source_call_occurrences
        .extend(source_call_occurrences);
    let applications = &mut lowered.semantic_module.closed_conformance_applications;
    applications.push(application);
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
    append_dynamic_dispatch(
        &mut lowered.semantic_module.dynamic_dispatch,
        dynamic_dispatch,
    );
    catalogs.scalar_calls.append_to(&mut lowered)?;
    finalize_operation_proofs(&mut lowered)?;
    retain_dynamic_source_owners(
        lowered,
        plan,
        &lowered_realizations,
        &forwarded_helpers,
        source_machine_ids,
    )
}

fn append_dynamic_dispatch(
    target: &mut TerminalDynamicDispatchCatalog,
    source: TerminalDynamicDispatchCatalog,
) {
    fn append<T: Ord>(target: &mut Vec<T>, source: Vec<T>) {
        target.extend(source);
        target.sort();
    }
    append(&mut target.parameters, source.parameters);
    append(&mut target.arguments, source.arguments);
    append(&mut target.selections, source.selections);
    append(&mut target.rebound_descriptors, source.rebound_descriptors);
    append(&mut target.stored_descriptors, source.stored_descriptors);
    append(&mut target.direct_dispatches, source.direct_dispatches);
    append(&mut target.indirect_dispatches, source.indirect_dispatches);
    append(&mut target.stored_dispatches, source.stored_dispatches);
    append(
        &mut target.parameter_dispatches,
        source.parameter_dispatches,
    );
}

fn empty_successor(target: BlockId, next_edge: &mut u64) -> Result<SuccessorEdge, LoweringError> {
    Ok(SuccessorEdge {
        edge: edge_id(allocate_dense(next_edge)?),
        target,
        arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    })
}

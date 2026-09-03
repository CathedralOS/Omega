//! Result-less lowering for the checked two-predecessor descriptor join.
//!
//! The descriptor/control structure is identical to the scalar join. Only the
//! branch calls, realization results, and helper calls are Unit-typed.

use super::*;

pub(super) fn lower(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedJoinedDynamicUnitCallPlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    validate_join_plan(checked, plan)?;
    let branches = [&plan.when_true.call, &plan.when_false.call];
    for branch in branches {
        unit::validate_exact_unit_plan(checked, branch, DynamicLoweringLane::Direct)?;
    }
    let first = branches[0];
    let second = branches[1];
    if first.caller_attachment_type_identity != second.caller_attachment_type_identity
        || first.caller_attachment_type_identity != plan.caller_attachment_type_identity
        || first.source_type_identity != second.source_type_identity
        || first.source_access != second.source_access
        || first.caller_parameter_access != second.caller_parameter_access
        || first.caller_multiplicity != second.caller_multiplicity
    {
        return unsupported("joined dynamic Unit branches do not share one caller ABI");
    }

    let (structural_types, type_ids) = lower_dynamic_structural_types_for_source(
        checked,
        &first.caller_attachment_type_identity,
        &first.caller_attachment_type_identity,
        &first.source_path,
        &first.source_type_identity,
    )?;
    let caller_attachment = lookup_type_id(&type_ids, &first.caller_attachment_type_identity)?;
    let source_type = lookup_type_id(&type_ids, &first.source_type_identity)?;
    let caller_access = match first.caller_parameter_access {
        CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
        CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
        _ => return unsupported("joined dynamic Unit caller requires borrowed self"),
    };
    let caller_self = StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: true,
        structural_type: caller_attachment,
        multiplicity: terminal_structural_multiplicity(first.caller_multiplicity),
        access: caller_access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let sources = [
        lower_source(&caller_self, first, &structural_types, &type_ids)?,
        lower_source(&caller_self, second, &structural_types, &type_ids)?,
    ];

    let mut lowered_realizations = joined_realizations(checked, &branches)?;
    for (index, realization) in lowered_realizations.iter_mut().enumerate() {
        realization.machine = machine_id(
            u64::try_from(index)
                .map_err(|_| {
                    LoweringError::Unsupported("joined Unit realization count exceeds u64")
                })?
                .checked_add(2)
                .ok_or(LoweringError::Unsupported(
                    "joined Unit realization identity overflowed",
                ))?,
        );
    }
    let branch_realizations = branches
        .iter()
        .map(|branch| realizations_for_plan(branch, &lowered_realizations))
        .collect::<Result<Vec<_>, _>>()?;
    let caller_machine = machine_id(1);
    let (first_application, first_row) = unit::lower_exact_unit_application(
        checked,
        first,
        caller_machine,
        &branch_realizations[0],
    )?;
    let (second_application, second_row) = unit::lower_exact_unit_application(
        checked,
        second,
        caller_machine,
        &branch_realizations[1],
    )?;
    let (requirements, requirement_slot) =
        dynamic_parameter_interface(&first_application, &first_row)?;
    let (second_requirements, second_slot) =
        dynamic_parameter_interface(&second_application, &second_row)?;
    if requirements != second_requirements
        || requirement_slot != second_slot
        || first_application.trait_identity != second_application.trait_identity
    {
        return unsupported("joined dynamic Unit conformances do not expose one exact interface");
    }

    let mut next_block = 4_u64;
    let mut next_place = 2_u64;
    let mut next_operation = 3_u64;
    let mut next_edge = 5_u64;
    let helper_ids = unit::forwarded_unit_helper_ids(
        first,
        &lowered_realizations,
        &mut next_block,
        &mut next_operation,
        &mut next_edge,
    )?;
    let first_helper = *helper_ids.first().ok_or(LoweringError::Unsupported(
        "joined dynamic Unit control has no forwarded helper",
    ))?;
    let helpers = unit::materialize_forwarded_unit_helper_chain(
        checked,
        first,
        &first_application,
        &first_row,
        &helper_ids,
    )?;

    let mut realization_machines = Vec::new();
    for realization in &lowered_realizations {
        let owner = branches
            .iter()
            .copied()
            .find(|branch| plan_contains_realization(branch, realization))
            .ok_or(LoweringError::Unsupported(
                "joined Unit realization has no checked branch owner",
            ))?;
        realization_machines.extend(unit::materialize_unit_realizations(
            checked,
            owner,
            std::slice::from_ref(realization),
            source_type,
            &mut next_block,
            &mut next_place,
            &mut next_edge,
        )?);
    }

    let mut dynamic_dispatch = TerminalDynamicDispatchCatalog {
        parameters: vec![TerminalDynamicDescriptorParameter {
            owner: first_helper.machine,
            ordinal: 0,
            source_position: 0,
            trait_identity: first_application.trait_identity.clone(),
            access: sources[0].access,
            requirements,
        }],
        arguments: vec![
            TerminalDynamicDescriptorArgument {
                owner: caller_machine,
                operation: operation_id(1),
                parameter_ordinal: 0,
                source: TerminalDynamicDescriptorSource::Selection { ordinal: 0 },
            },
            TerminalDynamicDescriptorArgument {
                owner: caller_machine,
                operation: operation_id(2),
                parameter_ordinal: 0,
                source: TerminalDynamicDescriptorSource::Selection { ordinal: 1 },
            },
        ],
        selections: vec![
            TerminalDynamicConformanceSelection {
                owner: caller_machine,
                ordinal: 0,
                source: sources[0].clone(),
                conformance_application_report_fingerprint: first_application.report_fingerprint,
                conformance_application_commitment: first_application.commitment,
            },
            TerminalDynamicConformanceSelection {
                owner: caller_machine,
                ordinal: 1,
                source: sources[1].clone(),
                conformance_application_report_fingerprint: second_application.report_fingerprint,
                conformance_application_commitment: second_application.commitment,
            },
        ],
        rebound_descriptors: Vec::new(),
        stored_descriptors: Vec::new(),
        direct_dispatches: Vec::new(),
        indirect_dispatches: Vec::new(),
        stored_dispatches: Vec::new(),
        parameter_dispatches: vec![TerminalParameterDynamicDispatch {
            owner: first_helper.machine,
            operation: first_helper.operation,
            parameter_ordinal: 0,
            requirement_slot,
        }],
    };
    unit::extend_unit_parameter_forwarding_catalog(&mut dynamic_dispatch, &helper_ids)?;

    let caller_reach = lower_installation_machine_service_ceiling(
        checked,
        first.caller_machine,
        checked
            .facts
            .service_reaches
            .plan_for_machine(first.caller_machine)
            .ok_or(LoweringError::Unsupported(
                "joined dynamic Unit caller has no checked service contract",
            ))?,
        exact_machine_service_summary(checked, first.caller_machine)?,
        &[],
    )?;
    let root_service_reach = lower_root_service_reach(checked, first.caller_machine, &[])?;
    let mut applications = vec![first_application, second_application];
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
    applications.dedup();
    let mut machines = vec![TerminalMachine {
        id: caller_machine,
        attachment: Some(caller_attachment),
        parameters: vec![ValueDeclaration {
            id: value_id(1),
            scalar_type: psi_core::ScalarType::Boolean,
        }],
        structural_parameters: vec![caller_self.clone()],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: caller_self.place,
            kind: StructuralPlaceKind::Parameter {
                position: caller_self.position,
                is_self: caller_self.is_self,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: caller_reach,
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![
            Block {
                id: block_id(1),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Conditional {
                    condition: value_id(1),
                    when_true: psi_terminal::SuccessorEdge {
                        edge: edge_id(1),
                        target: block_id(2),
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: psi_terminal::SuccessorEdge {
                        edge: edge_id(2),
                        target: block_id(3),
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            },
            branch_block(
                block_id(2),
                operation_id(1),
                edge_id(3),
                first_helper.machine,
            ),
            branch_block(
                block_id(3),
                operation_id(2),
                edge_id(4),
                first_helper.machine,
            ),
        ],
        contract: empty_terminal_contract(caller_machine.get()),
    }];
    machines.extend(realization_machines);
    machines.extend(helpers);
    machines.sort_by_key(|machine| machine.id);

    Ok(LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: caller_machine,
            structural_types,
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach,
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
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: applications,
            dynamic_dispatch,
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines,
        },
        proof_bundle: ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        },
        debug_map: None,
        source_call_occurrences: joined_source_call_occurrences(plan, &helper_ids)?,
        selected_ieee_float_fma_occurrences: Vec::new(),
    })
}

fn validate_join_plan(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedJoinedDynamicUnitCallPlan,
) -> Result<(), LoweringError> {
    if checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .joined_unit_calls
        .iter()
        .filter(|candidate| *candidate == plan)
        .count()
        != 1
    {
        return unsupported("joined dynamic Unit control plan drifted from checked custody");
    }
    join::validate_join_control_plan(
        checked,
        plan.caller_machine,
        plan.entry_state,
        &plan.scalar_parameters,
        &plan.guard,
        &plan.when_true.successor,
        plan.when_true.call.caller_machine,
        plan.when_true.call.caller_state,
        &plan.when_false.successor,
        plan.when_false.call.caller_machine,
        plan.when_false.call.caller_state,
    )
}

fn lower_source(
    caller_self: &StructuralParameterDeclaration,
    plan: &psi_checked_trees::CheckedDynamicUnitCallPlan,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    type_ids: &[(String, psi_core::StructuralTypeId)],
) -> Result<StructuralArgument, LoweringError> {
    validate_and_lower_dynamic_source(
        caller_self,
        plan.source_parameter_position,
        plan.caller_parameter_access,
        plan.caller_multiplicity,
        plan.source_access,
        &plan.caller_attachment_type_identity,
        &plan.source_path,
        &plan.source_type_identity,
        structural_types,
        type_ids,
    )
}

fn joined_realizations(
    checked: &CheckedTrees,
    branches: &[&psi_checked_trees::CheckedDynamicUnitCallPlan],
) -> Result<Vec<LoweredDynamicRealization>, LoweringError> {
    let mut joined = Vec::new();
    for branch in branches {
        for candidate in unit::collect_unit_realizations(checked, branch)? {
            if let Some(existing) = joined.iter().find(|existing: &&LoweredDynamicRealization| {
                existing.source_machine == candidate.source_machine
                    && existing.source_state == candidate.source_state
                    && existing.callable_identity == candidate.callable_identity
            }) {
                if existing.result != candidate.result {
                    return unsupported("joined dynamic Unit realization result drifted");
                }
            } else {
                joined.push(candidate);
            }
        }
    }
    if joined.is_empty() {
        return unsupported("joined dynamic Unit plan has no realizations");
    }
    Ok(joined)
}

fn realizations_for_plan(
    plan: &psi_checked_trees::CheckedDynamicUnitCallPlan,
    joined: &[LoweredDynamicRealization],
) -> Result<Vec<LoweredDynamicRealization>, LoweringError> {
    let retained = joined
        .iter()
        .filter(|realization| plan_contains_realization(plan, realization))
        .cloned()
        .collect::<Vec<_>>();
    if retained.len() != plan.realization_callables.len() {
        return unsupported("joined dynamic Unit conformance realization roster is incomplete");
    }
    Ok(retained)
}

fn plan_contains_realization(
    plan: &psi_checked_trees::CheckedDynamicUnitCallPlan,
    realization: &LoweredDynamicRealization,
) -> bool {
    plan.realization_callables.iter().any(|callable| {
        callable.realization_machine == realization.source_machine
            && callable.realization_state == realization.source_state
            && callable.realization_identity == realization.callable_identity
    })
}

fn branch_block(
    block: psi_core::BlockId,
    operation: psi_core::OperationId,
    edge: psi_core::EdgeId,
    callee: psi_core::MachineId,
) -> Block {
    Block {
        id: block,
        parameters: Vec::new(),
        operations: vec![Operation {
            id: operation,
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee,
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        }],
        terminator: Terminator::ReturnUnit {
            edge,
            trivial_affine_discards: Vec::new(),
        },
    }
}

fn joined_source_call_occurrences(
    plan: &psi_checked_trees::CheckedJoinedDynamicUnitCallPlan,
    helpers: &[unit::ForwardedUnitHelperIds],
) -> Result<Vec<LoweredSourceCallOccurrence>, LoweringError> {
    if helpers.len() != plan.when_true.call.forwarding_transfers.len() + 1
        || plan.when_true.call.forwarding_transfers != plan.when_false.call.forwarding_transfers
    {
        return unsupported("joined Unit source-call helper chain drifted from checked custody");
    }
    let join_state = plan
        .when_true
        .call
        .forwarding_transfers
        .first()
        .map(|transfer| transfer.caller_state);
    let mut occurrences = [
        (&plan.when_true.call, operation_id(1)),
        (&plan.when_false.call, operation_id(2)),
    ]
    .into_iter()
    .map(|(branch, operation)| {
        let psi_checked_trees::CheckedDynamicUnitCallOrigin::Forwarded { state, .. } =
            branch.origin
        else {
            return unsupported("joined Unit branch lost its forwarded source target");
        };
        Ok(LoweredSourceCallOccurrence {
            source_site: None,
            source_state: branch.caller_state,
            statement_index: usize::try_from(branch.coordinate.statement_index).map_err(|_| {
                LoweringError::Unsupported("joined Unit call statement exceeds usize")
            })?,
            call_ordinal: usize::try_from(branch.coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("joined Unit call ordinal exceeds usize")
            })?,
            terminal_operation: operation,
            source_target: join_state.unwrap_or(state),
            source_values_before_call: Vec::new(),
        })
    })
    .collect::<Result<Vec<_>, _>>()?;
    for (transfer, helper) in plan.when_true.call.forwarding_transfers.iter().zip(helpers) {
        occurrences.push(LoweredSourceCallOccurrence {
            source_site: None,
            source_state: transfer.caller_state,
            statement_index: usize::try_from(transfer.coordinate.statement_index).map_err(
                |_| LoweringError::Unsupported("joined Unit forwarding statement exceeds usize"),
            )?,
            call_ordinal: usize::try_from(transfer.coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("joined Unit forwarding call ordinal exceeds usize")
            })?,
            terminal_operation: helper.operation,
            source_target: transfer.target_state,
            source_values_before_call: Vec::new(),
        });
    }
    let psi_checked_trees::CheckedDynamicUnitCallOrigin::Forwarded {
        state, coordinate, ..
    } = plan.when_true.call.origin
    else {
        return unsupported("joined Unit dispatch lost its forwarded source coordinate");
    };
    occurrences.push(LoweredSourceCallOccurrence {
        source_site: None,
        source_state: state,
        statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported("joined Unit dispatch statement exceeds usize")
        })?,
        call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
            LoweringError::Unsupported("joined Unit dispatch ordinal exceeds usize")
        })?,
        terminal_operation: helpers
            .last()
            .ok_or(LoweringError::Unsupported(
                "joined Unit source-call chain has no final helper",
            ))?
            .operation,
        source_target: plan.when_true.call.requirement,
        source_values_before_call: Vec::new(),
    });
    Ok(occurrences)
}

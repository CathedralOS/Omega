//! First checked two-predecessor dynamic descriptor join.
//!
//! The runtime phi is the callee's ordinary dynamic descriptor parameter. Each
//! predecessor call supplies its own exact selection; no representative table
//! or joined-table vocabulary is introduced.

use super::*;

pub(super) fn lower(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedJoinedDynamicScalarCallPlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    validate_join_plan(checked, plan)?;
    let branches = [&plan.when_true.call, &plan.when_false.call];
    let first = branches[0];
    let second = branches[1];
    let first_shape = validate_exact_direct_plan(checked, first)?;
    let second_shape = validate_exact_direct_plan(checked, second)?;
    if first_shape.attachment_type_identity != second_shape.attachment_type_identity
        || first_shape.attachment_type_identity != plan.caller_attachment_type_identity
        || first.source_type_identity != second.source_type_identity
        || first.source_access != second.source_access
        || first.caller_parameter_access != second.caller_parameter_access
        || first.caller_multiplicity != second.caller_multiplicity
    {
        return unsupported("joined dynamic branches do not share one caller ABI");
    }

    let (structural_types, type_ids) =
        lower_dynamic_structural_types(checked, first, &first_shape.attachment_type_identity)?;
    let caller_attachment = lookup_type_id(&type_ids, &first_shape.attachment_type_identity)?;
    let source_type = lookup_type_id(&type_ids, &first.source_type_identity)?;
    let caller_access = match first.caller_parameter_access {
        CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
        CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
        _ => return unsupported("joined dynamic caller requires borrowed self"),
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
        validate_and_lower_source(&caller_self, first, &structural_types, &type_ids)?,
        validate_and_lower_source(&caller_self, second, &structural_types, &type_ids)?,
    ];

    let mut lowered_realizations = joined_realizations(checked, &branches)?;
    for (index, realization) in lowered_realizations.iter_mut().enumerate() {
        realization.machine = machine_id(
            u64::try_from(index)
                .map_err(|_| LoweringError::Unsupported("joined realization count exceeds u64"))?
                .checked_add(2)
                .ok_or(LoweringError::Unsupported(
                    "joined realization identity overflowed",
                ))?,
        );
    }
    let branch_realizations = branches
        .iter()
        .map(|branch| realizations_for_plan(branch, &lowered_realizations))
        .collect::<Result<Vec<_>, _>>()?;
    let caller_machine = machine_id(1);
    let (first_application, first_row) =
        lower_exact_application(checked, first, caller_machine, &branch_realizations[0])?;
    let (second_application, second_row) =
        lower_exact_application(checked, second, caller_machine, &branch_realizations[1])?;
    let (requirements, requirement_slot) =
        dynamic_parameter_interface(&first_application, &first_row)?;
    let (second_requirements, second_slot) =
        dynamic_parameter_interface(&second_application, &second_row)?;
    if requirements != second_requirements
        || requirement_slot != second_slot
        || first_application.trait_identity != second_application.trait_identity
    {
        return unsupported("joined dynamic conformances do not expose one exact interface");
    }

    let helper_machine = machine_id(
        u64::try_from(lowered_realizations.len())
            .map_err(|_| LoweringError::Unsupported("joined realization count exceeds u64"))?
            .checked_add(2)
            .ok_or(LoweringError::Unsupported(
                "joined helper identity overflowed",
            ))?,
    );
    let helper_ids = ForwardedHelperIds {
        machine: helper_machine,
        block: block_id(4),
        operation: operation_id(3),
        operation_value: value_id(5),
        result_value: value_id(4),
        edge: edge_id(5),
    };
    let helper = materialize_forwarded_helper_for_source(
        checked,
        first,
        &first_application,
        &first_row,
        helper_ids,
        forwarded_source_machine(first)?,
        None,
    )?;
    let result_type = terminal_scalar_type(first.result.primitive_type)?;
    let call_kind = || OperationKind::CallStructuralScalar {
        callee: helper_machine,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let caller_blocks = vec![
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
            value_id(2),
            edge_id(3),
            result_type,
            call_kind(),
        ),
        branch_block(
            block_id(3),
            operation_id(2),
            value_id(3),
            edge_id(4),
            result_type,
            call_kind(),
        ),
    ];

    let mut next_block = 5_u64;
    let mut next_place = 2_u64;
    let mut next_operation = 4_u64;
    let mut next_value = 6_u64;
    let mut next_edge = 6_u64;
    let mut realization_machines = Vec::new();
    for realization in &lowered_realizations {
        let owner = branches
            .iter()
            .copied()
            .find(|branch| plan_contains_realization(branch, realization))
            .ok_or(LoweringError::Unsupported(
                "joined realization has no checked branch owner",
            ))?;
        realization_machines.extend(materialize_dynamic_realizations(
            checked,
            owner,
            std::slice::from_ref(realization),
            source_type,
            &structural_types,
            &mut next_block,
            &mut next_place,
            &mut next_operation,
            &mut next_value,
            &mut next_edge,
        )?);
    }

    let selections = [
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
    ];
    let dynamic_dispatch = TerminalDynamicDispatchCatalog {
        parameters: vec![TerminalDynamicDescriptorParameter {
            owner: helper_machine,
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
        selections: selections.into(),
        rebound_descriptors: Vec::new(),
        stored_descriptors: Vec::new(),
        direct_dispatches: Vec::new(),
        indirect_dispatches: Vec::new(),
        stored_dispatches: Vec::new(),
        parameter_dispatches: vec![TerminalParameterDynamicDispatch {
            owner: helper_machine,
            operation: helper_ids.operation,
            parameter_ordinal: 0,
            requirement_slot,
        }],
    };
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

    let caller_reach = lower_installation_machine_service_ceiling(
        checked,
        first.caller_machine,
        checked
            .facts
            .service_reaches
            .plan_for_machine(first.caller_machine)
            .ok_or(LoweringError::Unsupported(
                "joined dynamic caller has no checked service contract",
            ))?,
        exact_machine_service_summary(checked, first.caller_machine)?,
        &[],
    )?;
    let root_service_reach = lower_root_service_reach(checked, first.caller_machine, &[])?;
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
        blocks: caller_blocks,
        contract: empty_terminal_contract(caller_machine.get()),
    }];
    machines.extend(realization_machines);
    machines.push(helper);
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
        source_call_occurrences: joined_source_call_occurrences(plan, helper_ids.operation)?,
        selected_ieee_float_fma_occurrences: Vec::new(),
    })
}

fn validate_join_plan(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedJoinedDynamicScalarCallPlan,
) -> Result<(), LoweringError> {
    if checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .joined_scalar_calls
        .iter()
        .filter(|candidate| *candidate == plan)
        .count()
        != 1
        || plan.when_true.successor.target_state != plan.when_true.call.caller_state
        || plan.when_false.successor.target_state != plan.when_false.call.caller_state
        || plan.when_true.call.caller_machine != plan.caller_machine
        || plan.when_false.call.caller_machine != plan.caller_machine
        || plan.when_true.successor.statement_ordinal != 0
        || plan.when_false.successor.statement_ordinal != 1
        || !plan.when_true.successor.transfers.is_empty()
        || !plan.when_false.successor.transfers.is_empty()
        || !plan.when_true.successor.scalar_arguments.is_empty()
        || !plan.when_false.successor.scalar_arguments.is_empty()
        || !plan
            .when_true
            .successor
            .trivial_affine_discard_parameter_positions
            .is_empty()
        || !plan
            .when_false
            .successor
            .trivial_affine_discard_parameter_positions
            .is_empty()
    {
        return unsupported("joined dynamic control plan drifted from checked custody");
    }
    let [parameter] = plan.scalar_parameters.as_slice() else {
        return unsupported("joined dynamic control requires one Boolean parameter");
    };
    if parameter.source_position != 1
        || parameter.primitive_type != PrimitiveType::Bool
        || !matches!(
            &plan.guard,
            CheckedScalarExpression::Boolean(boolean)
                if matches!(boolean.as_ref(), CheckedBooleanExpression::Parameter { position: 0 })
        )
    {
        return unsupported("joined dynamic control guard drifted from its Boolean input");
    }
    let states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter_map(|(_, state)| {
            (state.machine_symbol == plan.caller_machine && state.state_symbol == plan.entry_state)
                .then_some(state)
        })
        .collect::<Vec<_>>();
    let [entry] = states.as_slice() else {
        return unsupported("joined dynamic control lost its exact entry state");
    };
    let calls = checked.facts.flow.control.calls.span_or_empty(entry.calls);
    if calls.len() != 2
        || [
            (0_usize, plan.when_true.successor.target_state),
            (1_usize, plan.when_false.successor.target_state),
        ]
        .into_iter()
        .any(|(statement_index, target)| {
            calls
                .iter()
                .filter(|call| {
                    call.statement_index == statement_index
                        && call.call_ordinal == 0
                        && !call.has_receiver
                        && call.target_symbol == target
                })
                .count()
                != 1
        })
    {
        return unsupported("joined dynamic control successors drifted from checked flow");
    }
    Ok(())
}

fn joined_realizations(
    checked: &CheckedTrees,
    branches: &[&CheckedDynamicScalarCallPlan],
) -> Result<Vec<LoweredDynamicRealization>, LoweringError> {
    let mut joined = Vec::new();
    for branch in branches {
        for candidate in collect_dynamic_realizations(checked, branch)? {
            if let Some(existing) = joined.iter().find(|existing: &&LoweredDynamicRealization| {
                existing.source_machine == candidate.source_machine
                    && existing.source_state == candidate.source_state
                    && existing.callable_identity == candidate.callable_identity
            }) {
                if existing.result != candidate.result {
                    return unsupported("joined dynamic realization result drifted");
                }
            } else {
                joined.push(candidate);
            }
        }
    }
    if joined.is_empty() {
        return unsupported("joined dynamic plan has no realizations");
    }
    Ok(joined)
}

fn realizations_for_plan(
    plan: &CheckedDynamicScalarCallPlan,
    joined: &[LoweredDynamicRealization],
) -> Result<Vec<LoweredDynamicRealization>, LoweringError> {
    let retained = joined
        .iter()
        .filter(|realization| plan_contains_realization(plan, realization))
        .cloned()
        .collect::<Vec<_>>();
    if retained.len() != plan.realization_callables.len() {
        return unsupported("joined conformance realization roster is incomplete");
    }
    Ok(retained)
}

fn plan_contains_realization(
    plan: &CheckedDynamicScalarCallPlan,
    realization: &LoweredDynamicRealization,
) -> bool {
    plan.realization_callables.iter().any(|callable| {
        callable.realization_machine == realization.source_machine
            && callable.realization_state == realization.source_state
            && callable.realization_identity == realization.callable_identity
    })
}

fn forwarded_source_machine(
    plan: &CheckedDynamicScalarCallPlan,
) -> Result<psi_symbols::SymbolHandle, LoweringError> {
    let psi_checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { machine, .. } = plan.origin
    else {
        return unsupported("joined branch is not a forwarded dynamic call");
    };
    Ok(machine)
}

fn branch_block(
    block: psi_core::BlockId,
    operation: psi_core::OperationId,
    result: psi_core::ValueId,
    edge: psi_core::EdgeId,
    scalar_type: psi_core::ScalarType,
    kind: OperationKind,
) -> Block {
    Block {
        id: block,
        parameters: Vec::new(),
        operations: vec![Operation {
            id: operation,
            result: OperationResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type,
            }),
            kind,
        }],
        terminator: Terminator::ReturnUnit {
            edge,
            trivial_affine_discards: Vec::new(),
        },
    }
}

fn joined_source_call_occurrences(
    plan: &psi_checked_trees::CheckedJoinedDynamicScalarCallPlan,
    helper_operation: psi_core::OperationId,
) -> Result<Vec<LoweredSourceCallOccurrence>, LoweringError> {
    let branches = [
        (&plan.when_true.call, operation_id(1)),
        (&plan.when_false.call, operation_id(2)),
    ];
    let mut occurrences = branches
        .into_iter()
        .map(|(branch, operation)| {
            let psi_checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { state, .. } =
                branch.origin
            else {
                return unsupported("joined branch lost its forwarded source target");
            };
            Ok(LoweredSourceCallOccurrence {
                source_site: None,
                source_state: branch.caller_state,
                statement_index: usize::try_from(branch.coordinate.statement_index).map_err(
                    |_| LoweringError::Unsupported("joined call statement exceeds usize"),
                )?,
                call_ordinal: usize::try_from(branch.coordinate.call_ordinal)
                    .map_err(|_| LoweringError::Unsupported("joined call ordinal exceeds usize"))?,
                terminal_operation: operation,
                source_target: state,
                source_values_before_call: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let psi_checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
        state, coordinate, ..
    } = plan.when_true.call.origin
    else {
        return unsupported("joined dispatch lost its forwarded source coordinate");
    };
    occurrences.push(LoweredSourceCallOccurrence {
        source_site: None,
        source_state: state,
        statement_index: usize::try_from(coordinate.statement_index)
            .map_err(|_| LoweringError::Unsupported("joined dispatch statement exceeds usize"))?,
        call_ordinal: usize::try_from(coordinate.call_ordinal)
            .map_err(|_| LoweringError::Unsupported("joined dispatch ordinal exceeds usize"))?,
        terminal_operation: helper_operation,
        source_target: plan.when_true.call.requirement,
        source_values_before_call: Vec::new(),
    });
    Ok(occurrences)
}

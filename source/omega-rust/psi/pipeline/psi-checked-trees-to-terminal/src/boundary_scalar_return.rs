//! Result-bearing boundary custody lowering.

use super::*;

pub(super) fn lower_boundary_scalar_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedBoundaryScalarReturnMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let plans = &checked.facts.flow.terminal_boundary_scalar_returns;
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        scalar_arguments,
        structural_arguments,
        completion_receipts,
    } = &plan.boundary_call
    else {
        return unsupported("result-bearing boundary plan does not contain a boundary call");
    };
    if coordinate.statement_index != 0
        || coordinate.call_ordinal != 0
        || plan.return_statement_ordinal != 1
    {
        return unsupported("result-bearing boundary call coordinates are not canonical");
    }
    let mut matches = plans
        .boundary_machines
        .iter()
        .filter(|boundary| boundary.machine == *target_machine);
    let boundary = matches.next().ok_or(LoweringError::Unsupported(
        "result-bearing boundary target is absent from its checked plan",
    ))?;
    if matches.next().is_some()
        || boundary.state != *target_state
        || boundary.contract_report_fingerprint != *target_contract_report_fingerprint
        || boundary.result_type != Some(plan.result_type)
        || !checked_unit_target_reach_matches(*service_reach, boundary.contract_service_reach)
    {
        return unsupported("result-bearing boundary call disagrees with its exact checked target");
    }
    let exact_identity = checked
        .facts
        .contract_plans
        .for_machine(boundary.contract_owner)
        .map(|contract| (contract.report_fingerprint, contract.commitment))
        .or_else(|| {
            checked
                .facts
                .contract_plans
                .crash_capsule(boundary.contract_owner, boundary.state)
                .map(|capsule| {
                    (
                        capsule.target_contract_report_fingerprint(),
                        capsule.target_contract_commitment(),
                    )
                })
        })
        .ok_or(LoweringError::Unsupported(
            "result-bearing boundary target is missing its canonical contract identity",
        ))?;
    if (
        boundary.contract_report_fingerprint,
        boundary.contract_commitment,
    ) != exact_identity
    {
        return unsupported(
            "result-bearing boundary target contract compatibility coordinate or strong commitment drifted",
        );
    }

    let (structural_types, type_ids) = lower_structural_type_plans(&plans.structural_types)?;
    let (structural_domains, domain_ids) =
        lower_boundary_scalar_domains(checked, plans, plan, boundary, &type_ids)?;
    let (services, service_ids) =
        lower_boundary_scalar_services(checked, plan, boundary, *service_reach)?;
    let root_service_reach = lower_root_service_reach(checked, plan.machine, &service_ids)?;
    let mut next_place = 1_u64;
    let parameters = lower_unit_parameters(
        &plan.structural_parameters,
        &type_ids,
        &domain_ids,
        &mut next_place,
    )?;
    let boundary_parameters = lower_unit_parameters(
        &boundary.structural_parameters,
        &type_ids,
        &domain_ids,
        &mut next_place,
    )?;
    let boundary_scalar_parameters = boundary
        .scalar_parameters
        .iter()
        .map(|parameter| terminal_scalar_type(parameter.primitive_type))
        .collect::<Result<Vec<_>, _>>()?;
    let mut requires = boundary
        .domain_requirements
        .iter()
        .map(|requirement| {
            Ok(StructuralDomainRequirement {
                argument_index: requirement.argument_index,
                domain: lookup_domain_id(&domain_ids, requirement.domain)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    requires.sort();
    requires.dedup();
    let boundary_id = boundary_machine_id(1);
    let boundary_declaration = BoundaryMachineDeclaration {
        id: boundary_id,
        identity: checked_unit_boundary_identity(checked, boundary.machine)?,
        attachment: boundary
            .attachment_type_identity
            .as_ref()
            .map(|identity| lookup_type_id(&type_ids, identity))
            .transpose()?,
        scalar_parameters: boundary_scalar_parameters.clone(),
        structural_parameters: boundary_parameters,
        result: Some(terminal_scalar_type(plan.result_type)?),
        requires,
        program_local_root_introductions: Vec::new(),
        content_guarantees: lower_boundary_content_guarantees(
            &checked.facts.qualifications.content.conservation_plans,
            boundary.state,
        )?,
        published_service_ceiling: lower_published_service_ceiling(
            &checked.facts.service_reaches.rows,
            boundary.contract_service_reach,
            boundary.service_reach,
            &service_ids,
        )?,
    };

    let mut next_claim = 1_u64;
    let mut entry_claims = Vec::with_capacity(plan.entry_claims.len());
    let mut claim_bindings = Vec::with_capacity(plan.entry_claims.len());
    for claim in &plan.entry_claims {
        if claim.carry != CarryPolicy::STRICT {
            return unsupported("result-bearing boundary entry claim has non-default carry");
        }
        let parameter = parameters
            .get(usize::try_from(claim.parameter_index).map_err(|_| {
                LoweringError::Unsupported("boundary entry claim parameter exceeds usize")
            })?)
            .ok_or(LoweringError::Unsupported(
                "result-bearing boundary entry claim has an invalid parameter",
            ))?;
        let PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source: psi_language_semantics::PermissionEventSource::StateEntry,
            ..
        } = claim.claim_identity
        else {
            return unsupported("result-bearing boundary entry claim is not exact");
        };
        if machine_symbol != plan.machine || state_symbol != plan.state {
            return unsupported("result-bearing boundary entry claim belongs to another state");
        }
        let id = claim_id(allocate_dense(&mut next_claim)?);
        entry_claims.push(EntryClaim {
            claim: id,
            input: parameter.place,
            path: lower_structural_path(&claim.path),
        });
        claim_bindings.push((claim.claim_identity, id));
    }
    let expected_claim_arguments = structural_arguments
        .iter()
        .enumerate()
        .flat_map(|(argument_index, argument)| {
            plan.entry_claims
                .iter()
                .filter(move |claim| {
                    claim.parameter_index == argument.source_parameter_index
                        && (argument.path.is_empty() || claim.path == argument.path)
                })
                .map(move |_| {
                    u32::try_from(argument_index).map_err(|_| {
                        LoweringError::Unsupported("boundary argument index exceeds u32")
                    })
                })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    validate_transfer_shape(
        structural_arguments,
        completion_receipts,
        &parameters,
        &boundary.structural_parameters,
        &type_ids,
        &structural_types,
        &expected_claim_arguments,
    )?;
    if scalar_arguments.len() != boundary_scalar_parameters.len() {
        return unsupported(
            "result-bearing boundary scalar argument count disagrees with its declaration",
        );
    }
    let mut operations = OperationBuffer::new(0);
    let mut next_value_identity = 1_u64;
    let arguments = scalar_arguments
        .iter()
        .zip(&boundary_scalar_parameters)
        .map(|(argument, target_type)| {
            let argument = lower_checked_scalar_expression(argument)?;
            if argument.scalar_type() != *target_type {
                return unsupported(
                    "result-bearing boundary scalar argument type disagrees with its declaration",
                );
            }
            Ok(emit_direct_expression(
                &argument,
                &[],
                &mut next_value_identity,
                &mut operations,
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let scalar_type = terminal_scalar_type(plan.result_type)?;
    let call_result = ValueDeclaration {
        id: value_id(next_value_identity),
        scalar_type,
    };
    next_value_identity = next_value_identity
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "result-bearing boundary value identity space is exhausted",
        ))?;
    let operation = Operation {
        id: operations.allocate(),
        result: psi_terminal::OperationResult::Scalar(call_result),
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id,
            arguments,
            structural_arguments: lower_structural_arguments(
                structural_arguments,
                &parameters,
                &[],
            )?,
            completion_receipts: completion_receipts
                .iter()
                .map(|receipt| {
                    Ok(CompletionReceipt {
                        claim: lookup_claim_id(&claim_bindings, receipt.claim_identity)?,
                        argument_index: receipt.argument_index,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
            requirement_obligations: Vec::new(),
        },
    };
    operations.push(operation);
    let machine_result = ValueDeclaration {
        id: value_id(next_value_identity),
        scalar_type,
    };
    let content_entry_claims = content_conservation::lower_whole_content_entry_claims(
        checked,
        &plan.structural_parameters,
        &parameters,
        &plan.entry_claims,
        &claim_bindings,
    )?;
    let machine = TerminalMachine {
        id: machine_id(1),
        attachment: Some(lookup_type_id(&type_ids, &plan.attachment_type_identity)?),
        parameters: Vec::new(),
        structural_parameters: parameters.clone(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(machine_result),
        structural_places: parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
            })
            .collect(),
        entry_claims,
        published_service_ceiling: lower_installation_machine_service_ceiling(
            checked,
            plan.machine,
            plan.contract_service_reach,
            plan.service_reach,
            &service_ids,
        )?,
        content_entry_claims,
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: operations.operations,
            terminator: Terminator::Return {
                edge: edge_id(1),
                value: call_result.id,
                cleanup_actions: Vec::new(),
            },
        }],
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
            structural_domains,
            services,
            root_service_reach,
            placed_view_inputs: Vec::new(),
            boundary_machines: vec![boundary_declaration],
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
    };
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

fn lower_boundary_scalar_domains(
    checked: &CheckedTrees,
    plans: &psi_checked_trees::CheckedBoundaryScalarReturnPlans,
    machine: &CheckedBoundaryScalarReturnMachinePlan,
    boundary: &CheckedBoundaryMachinePlan,
    type_ids: &[(String, StructuralTypeId)],
) -> Result<
    (
        Vec<StructuralDomainDeclaration>,
        Vec<(SemanticDomainId, StructuralDomainId)>,
    ),
    LoweringError,
> {
    let mut selected = machine
        .structural_parameters
        .iter()
        .flat_map(|parameter| parameter.qualifications.iter().copied())
        .chain(
            boundary
                .structural_parameters
                .iter()
                .flat_map(|parameter| parameter.qualifications.iter().copied()),
        )
        .chain(
            boundary
                .domain_requirements
                .iter()
                .map(|requirement| requirement.domain),
        )
        .collect::<Vec<_>>();
    selected.sort_by_key(|domain| domain.0);
    selected.dedup();
    let mut selected_plans = selected
        .iter()
        .map(|domain| {
            plans
                .structural_domains
                .iter()
                .find(|plan| plan.domain == *domain)
                .ok_or(LoweringError::Unsupported(
                    "result-bearing boundary references a missing structural domain",
                ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    selected_plans.sort_by(|left, right| left.identity.cmp(&right.identity));
    if selected_plans
        .windows(2)
        .any(|pair| pair[0].identity == pair[1].identity)
    {
        return unsupported("result-bearing boundary has duplicate structural domains");
    }
    let domain_ids = selected_plans
        .iter()
        .enumerate()
        .map(|(index, plan)| Ok((plan.domain, structural_domain_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let declarations = selected_plans
        .into_iter()
        .map(|plan| {
            Ok(StructuralDomainDeclaration {
                id: lookup_domain_id(&domain_ids, plan.domain)?,
                semantic_domain: DomainSemanticId::new(u64::from(plan.domain.0))
                    .ok_or(LoweringError::InvalidContentDomainIdentity)?,
                identity: plan.identity.clone(),
                carrier: lookup_type_id(type_ids, &plan.carrier_type_identity)?,
                content_projection: content_conservation::lower_structural_content_projection(
                    checked,
                    plan.domain,
                    &plan.carrier_type_identity,
                )?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok((declarations, domain_ids))
}

fn lower_boundary_scalar_services(
    checked: &CheckedTrees,
    machine: &CheckedBoundaryScalarReturnMachinePlan,
    boundary: &CheckedBoundaryMachinePlan,
    call_reach: ServiceReachSummary,
) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError> {
    let facts = &checked.facts.service_reaches;
    let mut selected = Vec::new();
    collect_installation_machine_contract_services(
        checked,
        machine.machine,
        machine.contract_service_reach,
        machine.service_reach,
        &mut selected,
    )?;
    collect_published_contract_services(
        &facts.rows,
        boundary.contract_service_reach,
        boundary.service_reach,
        &mut selected,
    )?;
    collect_service_summary(&facts.rows, call_reach, &mut selected)?;
    let mut next = 0;
    while let Some(service) = selected.get(next).copied() {
        next += 1;
        let definition = facts
            .services
            .definition(service)
            .ok_or(LoweringError::Unsupported(
                "result-bearing boundary references an unknown service",
            ))?;
        for parent in &definition.parents {
            if !selected.contains(parent) {
                selected.push(*parent);
            }
        }
    }
    let mut definitions = selected
        .into_iter()
        .map(|service| {
            facts
                .services
                .definition(service)
                .map(|definition| (service, definition))
                .ok_or(LoweringError::Unsupported(
                    "result-bearing boundary references an unknown service",
                ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    definitions.sort_by(|left, right| left.1.name.cmp(&right.1.name));
    if definitions
        .windows(2)
        .any(|pair| pair[0].1.name == pair[1].1.name)
    {
        return unsupported("result-bearing boundary has duplicate service identities");
    }
    let service_ids = definitions
        .iter()
        .enumerate()
        .map(|(index, (source, _))| Ok((*source, service_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let declarations = definitions
        .into_iter()
        .map(|(source, definition)| {
            let mut parents = definition
                .parents
                .iter()
                .map(|parent| lookup_service_id(&service_ids, *parent))
                .collect::<Result<Vec<_>, LoweringError>>()?;
            parents.sort();
            parents.dedup();
            Ok(ServiceDeclaration {
                id: lookup_service_id(&service_ids, source)?,
                identity: definition.name.clone(),
                parents,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok((declarations, service_ids))
}

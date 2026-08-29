//! Attached Unit closure assembly.
//!
//! The orchestrator retains exact transitive closure and publication order;
//! provider discovery, call-closure custody, semantic catalogs, and parameter
//! transfer validation live in separate subordinate modules.

use super::*;

mod call_closure;
mod catalog;
mod parameters;
mod providers;

use call_closure::{
    checked_terminal_machine_name, reject_recursive_unit_closure, unique_unit_boundary,
    validate_unit_operation_sequence,
};
pub(super) use call_closure::{
    checked_unit_boundary_identity, checked_unit_call_closure_including, unique_unit_machine,
};
pub(super) use catalog::{
    checked_unit_target_reach_matches, collect_contract_services,
    collect_installation_machine_contract_services, collect_service_summary,
    lower_root_service_reach,
};
use catalog::{
    lower_program_local_root_introductions, lower_provider_candidate_service_ceiling,
    lower_unit_services, lower_unit_structural_domains, lower_unit_structural_types,
    require_valid_service_row,
};
pub(super) use parameters::{
    lower_installation_machine_service_ceiling, lower_published_service_ceiling,
    lower_structural_arguments, lower_structural_path, lower_unit_parameters,
    validate_transfer_shape,
};
use providers::checked_unit_provider_candidates;

pub(super) fn lower_attached_unit_closure(
    checked: &CheckedTrees,
    entry: psi_symbols::SymbolHandle,
) -> Result<LoweredTerminalPsi, LoweringError> {
    lower_attached_unit_closure_including(checked, entry, &[])
}

pub(super) fn lower_attached_unit_closure_including(
    checked: &CheckedTrees,
    entry: psi_symbols::SymbolHandle,
    additional_roots: &[psi_symbols::SymbolHandle],
) -> Result<LoweredTerminalPsi, LoweringError> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut retained_roots = additional_roots.to_vec();
    let (closure, provider_candidate_plans) = loop {
        let closure = checked_unit_call_closure_including(checked, entry, &retained_roots)?;
        let candidates = checked_unit_provider_candidates(checked, &closure)?;
        let new_roots = candidates
            .iter()
            .map(|candidate| candidate.candidate)
            .filter(|candidate| !retained_roots.contains(candidate) && *candidate != entry)
            .collect::<Vec<_>>();
        if new_roots.is_empty() {
            break (closure, candidates);
        }
        retained_roots.extend(new_roots);
    };
    reject_recursive_unit_closure(plans, &closure)?;

    let mut boundaries = Vec::<(&CheckedBoundaryMachinePlan, String)>::new();
    for machine_symbol in &closure {
        let machine = unique_unit_machine(plans, *machine_symbol)?;
        if machine.contract_report_fingerprint == 0 {
            return unsupported("Unit closure contains a null checked contract fingerprint");
        }
        let contract = checked
            .facts
            .contract_plans
            .for_machine(machine.machine)
            .ok_or(LoweringError::Unsupported(
                "Unit closure is missing its canonical checked contract",
            ))?;
        if machine.contract_report_fingerprint != contract.report_fingerprint
            || machine.contract_commitment != contract.commitment
        {
            return unsupported(
                "Unit closure contract compatibility coordinate or strong commitment drifted",
            );
        }
        validate_unit_operation_sequence(machine)?;
        for operation in &machine.operations {
            match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    service_reach,
                    ..
                } => {
                    let target = unique_unit_machine(plans, *target_machine)?;
                    if target.state != *target_state
                        || target.contract_report_fingerprint != *target_contract_report_fingerprint
                        || !checked_unit_target_reach_matches(
                            *service_reach,
                            target.contract_service_reach,
                        )
                    {
                        return unsupported(
                            "Unit call does not match the exact checked target state, contract, and reach",
                        );
                    }
                }
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    service_reach,
                    ..
                } => {
                    let target = unique_unit_boundary(plans, *target_machine)?;
                    if target.contract_report_fingerprint == 0 {
                        return unsupported(
                            "Unit boundary target has a null checked contract fingerprint",
                        );
                    }
                    if target.state != *target_state
                        || target.contract_report_fingerprint != *target_contract_report_fingerprint
                        || !checked_unit_target_reach_matches(
                            *service_reach,
                            target.contract_service_reach,
                        )
                    {
                        return unsupported(
                            "boundary Unit call does not match the exact checked target state, contract, and reach",
                        );
                    }
                    let exact_identity = checked
                        .facts
                        .contract_plans
                        .for_machine(target.contract_owner)
                        .map(|contract| (contract.report_fingerprint, contract.commitment))
                        .or_else(|| {
                            checked
                                .facts
                                .contract_plans
                                .crash_capsule(target.contract_owner, target.state)
                                .map(|capsule| {
                                    (
                                        capsule.target_contract_report_fingerprint(),
                                        capsule.target_contract_commitment(),
                                    )
                                })
                        })
                        .ok_or(LoweringError::Unsupported(
                            "Unit boundary target is missing its canonical checked contract identity",
                        ))?;
                    if (
                        target.contract_report_fingerprint,
                        target.contract_commitment,
                    ) != exact_identity
                    {
                        return unsupported(
                            "Unit boundary target contract compatibility coordinate or strong commitment drifted",
                        );
                    }
                    if !boundaries
                        .iter()
                        .any(|(candidate, _)| candidate.machine == target.machine)
                    {
                        boundaries.push((
                            target,
                            checked_unit_boundary_identity(checked, target.machine)?,
                        ));
                    }
                }
                CheckedUnitEffectOperationPlan::PortWrite { .. }
                | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                | CheckedUnitEffectOperationPlan::ReturnUnit { .. } => {}
            }
        }
    }
    boundaries.sort_by(|left, right| left.1.cmp(&right.1));
    if boundaries.windows(2).any(|pair| pair[0].1 == pair[1].1) {
        return unsupported("boundary Unit closure contains duplicate canonical identities");
    }

    let (structural_types, type_ids) = lower_unit_structural_types(checked, &closure, &boundaries)?;
    let (structural_domains, domain_ids) =
        lower_unit_structural_domains(checked, &closure, &boundaries, &type_ids)?;
    let (services, service_ids) =
        lower_unit_services(checked, &closure, &boundaries, &provider_candidate_plans)?;
    let root_service_reach = lower_root_service_reach(checked, entry, &service_ids)?;

    let mut next_place = 1_u64;
    let mut lowered_boundary_parameters = Vec::with_capacity(boundaries.len());
    let mut boundary_machines = Vec::with_capacity(boundaries.len());
    for (index, (plan, identity)) in boundaries.iter().enumerate() {
        let parameters = lower_unit_parameters(
            &plan.structural_parameters,
            &type_ids,
            &domain_ids,
            &mut next_place,
        )?;
        let scalar_parameters = plan
            .scalar_parameters
            .iter()
            .map(|parameter| terminal_scalar_type(parameter.primitive_type))
            .collect::<Result<Vec<_>, _>>()?;
        let mut requires = plan
            .domain_requirements
            .iter()
            .map(|requirement| {
                if usize::try_from(requirement.argument_index)
                    .ok()
                    .map_or(true, |index| index >= parameters.len())
                {
                    return Err(LoweringError::Unsupported(
                        "boundary structural requirement has an invalid argument index",
                    ));
                }
                Ok(StructuralDomainRequirement {
                    argument_index: requirement.argument_index,
                    domain: lookup_domain_id(&domain_ids, requirement.domain)?,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        requires.sort();
        let original_requirement_count = requires.len();
        requires.dedup();
        if requires.len() != original_requirement_count {
            return unsupported("boundary structural requirements contain duplicates");
        }
        let published_service_ceiling = lower_published_service_ceiling(
            &checked.facts.service_reaches.rows,
            plan.contract_service_reach,
            plan.service_reach,
            &service_ids,
        )?;
        let id = boundary_machine_id(dense_identity(index)?);
        let program_local_root_introductions = lower_program_local_root_introductions(
            checked,
            plan,
            identity,
            &parameters,
            &domain_ids,
        )?;
        let content_guarantees = lower_boundary_content_guarantees(
            &checked.facts.qualifications.content.conservation_plans,
            plan.state,
        )?;
        boundary_machines.push(BoundaryMachineDeclaration {
            id,
            identity: identity.clone(),
            attachment: plan
                .attachment_type_identity
                .as_ref()
                .map(|identity| lookup_type_id(&type_ids, identity))
                .transpose()?,
            scalar_parameters: scalar_parameters.clone(),
            structural_parameters: parameters.clone(),
            result: plan.result_type.map(terminal_scalar_type).transpose()?,
            requires,
            program_local_root_introductions,
            content_guarantees,
            published_service_ceiling,
        });
        lowered_boundary_parameters.push((plan.machine, id, parameters, scalar_parameters));
    }

    let mut lowered_machine_parameters = Vec::with_capacity(closure.len());
    let mut lowered_claims = Vec::with_capacity(closure.len());
    for machine_symbol in &closure {
        let plan = unique_unit_machine(plans, *machine_symbol)?;
        if plan.body_qualifications.iter().any(|domain| {
            !plan
                .structural_parameters
                .iter()
                .any(|parameter| parameter.qualifications.contains(domain))
        }) {
            return unsupported(
                "Unit body qualification is not represented by an exact structural parameter precondition",
            );
        }
        let parameters = lower_unit_parameters(
            &plan.structural_parameters,
            &type_ids,
            &domain_ids,
            &mut next_place,
        )?;
        let mut claims = Vec::with_capacity(plan.entry_claims.len());
        let mut claim_bindings = Vec::with_capacity(plan.entry_claims.len());
        // ClaimId is machine-local; unrelated closure members must not shift
        // this machine's canonical claim namespace.
        let mut next_claim = 1_u64;
        for claim in &plan.entry_claims {
            if claim.carry != CarryPolicy::STRICT {
                return unsupported("Unit entry claim has a non-default carry policy");
            }
            let parameter = parameters
                .get(usize::try_from(claim.parameter_index).map_err(|_| {
                    LoweringError::Unsupported("Unit entry claim parameter index exceeds usize")
                })?)
                .ok_or(LoweringError::Unsupported(
                    "Unit entry claim has an invalid parameter index",
                ))?;
            let PermissionClaimIdentity::Established {
                machine_symbol,
                state_symbol,
                source: psi_language_semantics::PermissionEventSource::StateEntry,
                ..
            } = claim.claim_identity
            else {
                return unsupported("Unit entry claim is not an exact checked state-entry claim");
            };
            if machine_symbol != plan.machine || state_symbol != plan.state {
                return unsupported("Unit entry claim belongs to another checked state");
            }
            if claim_bindings
                .iter()
                .any(|(identity, _)| *identity == claim.claim_identity)
            {
                return unsupported("Unit entry claim identity is duplicated");
            }
            let id = claim_id(allocate_dense(&mut next_claim)?);
            claims.push(EntryClaim {
                claim: id,
                input: parameter.place,
                path: lower_structural_path(&claim.path),
            });
            claim_bindings.push((claim.claim_identity, id));
        }
        lowered_machine_parameters.push((*machine_symbol, parameters));
        lowered_claims.push((*machine_symbol, claims, claim_bindings));
    }

    let lowered_machine_runtime_requirements = closure
        .iter()
        .map(|machine_symbol| {
            let Some(contract) = checked.facts.contract_plans.for_machine(*machine_symbol) else {
                return Ok((*machine_symbol, Vec::new()));
            };
            let requirements = if contract.crash.uses_structural_proof_gated_arithmetic() {
                let checked_requirements = contract.crash.structural_runtime_requirements().ok_or(
                    LoweringError::Unsupported(
                        "proof-gated structural arithmetic lacks a complete checked requirement package",
                    ),
                )?;
                let parameters = lowered_machine_parameters
                    .iter()
                    .find_map(|(symbol, parameters)| {
                        (*symbol == *machine_symbol).then_some(parameters)
                    })
                    .expect("every closure machine has lowered parameters");
                let requirements = checked_requirements
                    .iter()
                    .map(|requirement| {
                        lower_structural_runtime_requirement(
                            requirement,
                            parameters,
                            &structural_types,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut keyed = requirements
                    .into_iter()
                    .map(|requirement| {
                        psi_terminal_codec::canonical_proposition_order_key(&requirement)
                            .map(|key| (key, requirement))
                            .map_err(|_| {
                                LoweringError::Unsupported(
                                    "structural runtime requirement is not canonically encodable",
                                )
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                keyed.sort_by(|left, right| left.0.cmp(&right.0));
                keyed.dedup_by(|left, right| left.0 == right.0);
                keyed
                    .into_iter()
                    .map(|(_, requirement)| requirement)
                    .collect()
            } else {
                Vec::new()
            };
            Ok((*machine_symbol, requirements))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;

    let machine_ids = closure
        .iter()
        .enumerate()
        .map(|(index, symbol)| Ok((*symbol, machine_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_operation = 1_u64;
    let mut next_edge = 1_u64;
    let mut next_block = 1_u64;
    let mut next_call_obligation = TERMINAL_UNIT_CALL_OBLIGATION_BASE;
    let mut call_evidence = Vec::new();
    let mut machines = Vec::with_capacity(closure.len());

    for machine_symbol in &closure {
        let plan = unique_unit_machine(plans, *machine_symbol)?;
        let terminal_machine = lookup_machine_id(&machine_ids, plan.machine)?;
        let parameters = lowered_machine_parameters
            .iter()
            .find_map(|(symbol, parameters)| (*symbol == plan.machine).then_some(parameters))
            .expect("every closure machine has lowered parameters");
        let runtime_requirements = lowered_machine_runtime_requirements
            .iter()
            .find_map(|(symbol, requirements)| (*symbol == plan.machine).then_some(requirements))
            .expect("every closure machine has lowered runtime requirements");
        let (_, entry_claims, claim_bindings) = lowered_claims
            .iter()
            .find(|(symbol, _, _)| *symbol == plan.machine)
            .expect("every closure machine has lowered entry claims");
        let content_entry_claims = content_conservation::lower_whole_content_entry_claims(
            checked,
            &plan.structural_parameters,
            parameters,
            &plan.entry_claims,
            claim_bindings,
        )?;
        let attachment = lookup_type_id(&type_ids, &plan.attachment_type_identity)?;
        let checked_attachment = plans
            .structural_types
            .iter()
            .find(|declaration| declaration.identity == plan.attachment_type_identity)
            .ok_or(LoweringError::Unsupported(
                "attached Unit machine is missing its checked attachment shape",
            ))?;
        let checked_provider_fields = match &checked_attachment.shape {
            CheckedUnitStructuralTypeShape::Record { fields } => fields
                .iter()
                .filter(|field| {
                    matches!(
                        field.field_type,
                        CheckedUnitStructuralFieldType::ProviderBacked { .. }
                    )
                })
                .count(),
            _ => 0,
        };
        if (checked_provider_fields == 0) != plan.provider_attachment_requirements.is_empty()
            || checked_provider_fields > 1
        {
            return unsupported(
                "provider-backed attachment field lacks one complete specialization requirement set",
            );
        }
        if checked_provider_fields == 1 {
            let mut called_boundaries = plan
                .operations
                .iter()
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } => {
                        Some(*target_machine)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            called_boundaries
                .sort_by_key(|boundary| (boundary.arena_index(), boundary.generation()));
            called_boundaries.dedup();
            let specialized_boundaries = plan
                .provider_attachment_requirements
                .iter()
                .map(|requirement| requirement.boundary)
                .collect::<Vec<_>>();
            if called_boundaries != specialized_boundaries {
                return unsupported(
                    "provider-backed attachment requirements do not exactly cover boundary calls",
                );
            }
        }
        let attachment_declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == attachment)
            .expect("lowered attachment declaration exists");
        let attachment_fields = match &attachment_declaration.shape {
            StructuralTypeShape::Record { fields } => Some(fields.as_slice()),
            _ => None,
        };
        if !plan.provider_attachment_requirements.is_empty() && attachment_fields.is_none() {
            return unsupported(
                "provider-backed attachment specialization requires a record attachment",
            );
        }
        let mut provider_roots = plan
            .provider_attachment_requirements
            .iter()
            .map(|requirement| {
                let field = attachment_fields
                    .expect("provider requirements require record attachment")
                    .iter()
                    .find(|field| field.identity == requirement.field_identity)
                    .ok_or(LoweringError::Unsupported(
                        "provider-backed attachment requirement names an unknown field",
                    ))?;
                if field.relevance.is_erased()
                    || !matches!(&field.field_type,
                        StructuralFieldType::Erased { type_identity }
                            if type_identity == &requirement.provider_type_identity)
                {
                    return unsupported(
                        "provider-backed attachment requirement disagrees with its erased carrier",
                    );
                }
                let boundary = lowered_boundary_parameters
                    .iter()
                    .find_map(|(symbol, boundary, _, _)| {
                        (*symbol == requirement.boundary).then_some(*boundary)
                    })
                    .ok_or(LoweringError::Unsupported(
                        "provider-backed attachment requirement names an unlowered boundary",
                    ))?;
                Ok((field.id, boundary))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        provider_roots.sort_by_key(|(_, boundary)| *boundary);
        if provider_roots.windows(2).any(|pair| pair[0] == pair[1]) {
            return unsupported("provider-backed attachment requirements contain duplicates");
        }
        let provider_places = provider_roots
            .into_iter()
            .map(|(field, boundary)| {
                Ok(StructuralPlaceDeclaration {
                    id: place_id(allocate_dense(&mut next_place)?),
                    kind: StructuralPlaceKind::ProviderAttachment {
                        attachment,
                        field,
                        boundary,
                    },
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let local_places = plan
            .trivial_affine_locals
            .iter()
            .map(|local| {
                Ok(StructuralPlaceDeclaration {
                    id: place_id(allocate_dense(&mut next_place)?),
                    kind: StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal: local.declaration_ordinal,
                        structural_type: lookup_type_id(&type_ids, &local.type_identity)?,
                    },
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let mut literal_arguments = Vec::new();
        for operation in &plan.operations {
            match operation {
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    structural_arguments,
                    ..
                } => literal_arguments.extend(
                    structural_arguments
                        .iter()
                        .filter(|argument| argument.byte_sequence_literal.is_some()),
                ),
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } if structural_arguments
                    .iter()
                    .any(|argument| argument.byte_sequence_literal.is_some()) =>
                {
                    return unsupported(
                        "byte-sequence literals may target only bodyless boundaries",
                    );
                }
                _ => {}
            }
        }
        let literal_places = literal_arguments
            .iter()
            .enumerate()
            .map(|(ordinal, argument)| {
                Ok(StructuralPlaceDeclaration {
                    id: place_id(allocate_dense(&mut next_place)?),
                    kind: StructuralPlaceKind::ByteSequenceLiteral {
                        declaration_ordinal: u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("byte-sequence literal count exceeds u32")
                        })?,
                        structural_type: lookup_type_id(&type_ids, &argument.type_identity)?,
                    },
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let operation_identity_base = next_operation
            .checked_sub(1)
            .expect("terminal operation identity starts at one");
        let mut operations = OperationBuffer::new(operation_identity_base);
        for (argument, place) in literal_arguments.iter().zip(&literal_places) {
            let bytes =
                argument
                    .byte_sequence_literal
                    .as_ref()
                    .ok_or(LoweringError::Unsupported(
                        "byte-sequence literal payload is absent",
                    ))?;
            let id = operations.allocate();
            operations.push(Operation {
                id,
                result: psi_terminal::OperationResult::Unit,
                kind: OperationKind::EstablishByteSequenceLiteral {
                    destination: place.id,
                    bytes: bytes.clone(),
                },
            });
        }
        let mut next_literal_argument = 0usize;
        let mut next_value_identity = 1_u64;
        for operation in &plan.operations[..plan.operations.len() - 1] {
            let kind = match operation {
                CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                    declaration_ordinal,
                    type_identity,
                    ..
                } => {
                    let local = local_places
                        .get(usize::try_from(*declaration_ordinal).map_err(|_| {
                            LoweringError::Unsupported("Unit local ordinal exceeds usize")
                        })?)
                        .ok_or(LoweringError::Unsupported(
                            "Unit local ordinal is not dense",
                        ))?;
                    if !matches!(
                        local.kind,
                        StructuralPlaceKind::TrivialAffineLocal {
                            declaration_ordinal: ordinal,
                            structural_type,
                        } if ordinal == *declaration_ordinal
                            && structural_type == lookup_type_id(&type_ids, type_identity)?
                    ) {
                        return unsupported("Unit local declaration drifted from checked custody");
                    }
                    OperationKind::EstablishTrivialAffineLocal {
                        destination: local.id,
                    }
                }
                CheckedUnitEffectOperationPlan::CallUnit {
                    target_machine,
                    structural_arguments,
                    claim_transfers,
                    ..
                } => {
                    let target = unique_unit_machine(plans, *target_machine)?;
                    validate_transfer_shape(
                        structural_arguments,
                        claim_transfers,
                        parameters,
                        &target.structural_parameters,
                        &type_ids,
                        &structural_types,
                        &target
                            .entry_claims
                            .iter()
                            .map(|claim| claim.parameter_index)
                            .collect::<Vec<_>>(),
                    )?;
                    let terminal_arguments =
                        lower_structural_arguments(structural_arguments, parameters, &[])?;
                    let target_parameters = lowered_machine_parameters
                        .iter()
                        .find_map(|(symbol, parameters)| {
                            (*symbol == *target_machine).then_some(parameters)
                        })
                        .expect("every closure target has lowered parameters");
                    let mut crash_continuations = if let Some(target_contract) =
                        checked.facts.contract_plans.for_machine(*target_machine)
                    {
                        lower_structural_crash_route_buckets(
                            target_contract.crash.published(),
                            target_parameters,
                            &structural_types,
                            lowered_machine_runtime_requirements
                                .iter()
                                .find_map(|(symbol, requirements)| {
                                    (*symbol == *target_machine).then_some(requirements.as_slice())
                                })
                                .expect("every closure target has lowered runtime requirements"),
                        )?
                    } else {
                        Vec::new()
                    };
                    let substitutions = target_parameters
                        .iter()
                        .zip(&terminal_arguments)
                        .map(|(parameter, argument)| {
                            Ok((
                                parameter.place,
                                (
                                    argument.place,
                                    structural_crash_route_argument_prefix(
                                        argument,
                                        parameters,
                                        &structural_types,
                                    )?,
                                ),
                            ))
                        })
                        .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
                    substitute_structural_crash_route_roots(
                        &mut crash_continuations,
                        &substitutions,
                    )?;
                    let target_runtime_requirements = lowered_machine_runtime_requirements
                        .iter()
                        .find_map(|(symbol, requirements)| {
                            (*symbol == *target_machine).then_some(requirements)
                        })
                        .expect("every closure target has lowered runtime requirements");
                    let requirement_obligations = target_runtime_requirements
                        .iter()
                        .map(|requirement| {
                            let mut goal = requirement.clone();
                            substitute_structural_requirement_roots(&mut goal, &substitutions)?;
                            let assumption_index = runtime_requirements
                                .iter()
                                .position(|assumption| assumption == &goal)
                                .ok_or(LoweringError::Unsupported(
                                    "runtime structural call requirement is not an exact caller premise",
                                ))?;
                            let obligation = obligation_id(next_call_obligation);
                            next_call_obligation = next_call_obligation
                                .checked_add(1)
                                .ok_or(LoweringError::Unsupported(
                                    "runtime structural call obligation identity space is exhausted",
                                ))?;
                            call_evidence.push(ObligationEvidence {
                                obligation,
                                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                                    identity: EvidenceIdentity::new(obligation.get())
                                        .expect("terminal obligation identity is nonzero"),
                                    proof_system_marker: ProofSystemMarker::CURRENT,
                                    proof: ProofNode {
                                        conclusion: goal,
                                        rule: ProofRule::Assumption {
                                            index: assumption_index,
                                        },
                                    },
                                }),
                            });
                            Ok(obligation)
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    OperationKind::CallUnit {
                        callee: lookup_machine_id(&machine_ids, *target_machine)?,
                        structural_arguments: terminal_arguments,
                        claim_transfers: claim_transfers
                            .iter()
                            .map(|transfer| {
                                Ok(ClaimTransfer {
                                    claim: lookup_claim_id(
                                        claim_bindings,
                                        transfer.claim_identity,
                                    )?,
                                    argument_index: transfer.argument_index,
                                })
                            })
                            .collect::<Result<Vec<_>, LoweringError>>()?,
                        requirement_obligations,
                        crash_continuations,
                    }
                }
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    target_machine,
                    scalar_arguments,
                    structural_arguments,
                    completion_receipts,
                    ..
                } => {
                    let target = unique_unit_boundary(plans, *target_machine)?;
                    let expected_claim_arguments = structural_arguments
                        .iter()
                        .enumerate()
                        .flat_map(|(argument_index, argument)| {
                            plan.entry_claims
                                .iter()
                                .filter(move |claim| {
                                    argument.byte_sequence_literal.is_none()
                                        && claim.parameter_index == argument.source_parameter_index
                                        && (argument.path.is_empty() || claim.path == argument.path)
                                })
                                .map(move |_| {
                                    u32::try_from(argument_index).map_err(|_| {
                                        LoweringError::Unsupported(
                                            "boundary Unit argument index exceeds u32",
                                        )
                                    })
                                })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    validate_transfer_shape(
                        structural_arguments,
                        completion_receipts,
                        parameters,
                        &target.structural_parameters,
                        &type_ids,
                        &structural_types,
                        &expected_claim_arguments,
                    )?;
                    let (_, boundary, _, target_scalar_parameters) = lowered_boundary_parameters
                        .iter()
                        .find(|(symbol, _, _, _)| *symbol == *target_machine)
                        .ok_or(LoweringError::Unsupported(
                            "boundary Unit call target is absent from the lowered closure",
                        ))?;
                    if scalar_arguments.len() != target_scalar_parameters.len() {
                        return unsupported(
                            "boundary Unit scalar argument count disagrees with its declaration",
                        );
                    }
                    let arguments = scalar_arguments
                        .iter()
                        .zip(target_scalar_parameters)
                        .map(|(argument, target_type)| {
                            let argument = lower_checked_scalar_expression(argument)?;
                            if argument.scalar_type() != *target_type {
                                return unsupported(
                                    "boundary Unit scalar argument type disagrees with its declaration",
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
                    let literal_count = structural_arguments
                        .iter()
                        .filter(|argument| argument.byte_sequence_literal.is_some())
                        .count();
                    let literal_end = next_literal_argument.checked_add(literal_count).ok_or(
                        LoweringError::Unsupported(
                            "byte-sequence literal argument count overflows usize",
                        ),
                    )?;
                    let call_literal_places = literal_places
                        .get(next_literal_argument..literal_end)
                        .ok_or(LoweringError::Unsupported(
                            "byte-sequence literal argument place is absent",
                        ))?
                        .iter()
                        .map(|place| place.id)
                        .collect::<Vec<_>>();
                    next_literal_argument = literal_end;
                    OperationKind::BoundaryCall {
                        boundary: *boundary,
                        arguments,
                        structural_arguments: lower_structural_arguments(
                            structural_arguments,
                            parameters,
                            &call_literal_places,
                        )?,
                        completion_receipts: completion_receipts
                            .iter()
                            .map(|settlement| {
                                Ok(CompletionReceipt {
                                    claim: lookup_claim_id(
                                        claim_bindings,
                                        settlement.claim_identity,
                                    )?,
                                    argument_index: settlement.argument_index,
                                })
                            })
                            .collect::<Result<Vec<_>, LoweringError>>()?,
                        requirement_obligations: Vec::new(),
                    }
                }
                CheckedUnitEffectOperationPlan::PortWrite {
                    service_reach,
                    port,
                    value,
                    ..
                } => {
                    let direct = checked
                        .facts
                        .service_reaches
                        .rows
                        .services(service_reach.direct);
                    let [port_service] = direct else {
                        return unsupported(
                            "port output does not carry the unique exact checked PortIo service",
                        );
                    };
                    if !checked
                        .facts
                        .service_reaches
                        .rows
                        .services(service_reach.transitive)
                        .contains(port_service)
                    {
                        return unsupported(
                            "port output does not carry the unique exact checked PortIo service",
                        );
                    }
                    OperationKind::PortWrite {
                        // `CheckedUnitEffectOperationPlan::PortWrite` is minted only for the
                        // exact checked asm-port-out builtin. Its singleton direct row is
                        // therefore the symbol-backed PortIo authority; no spelling lookup is
                        // repeated here.
                        service: lookup_service_id(&service_ids, *port_service)?,
                        port: *port,
                        value: *value,
                    }
                }
                CheckedUnitEffectOperationPlan::ReturnUnit { .. } => {
                    return unsupported("Unit return is not the final checked operation");
                }
            };
            let id = operations.allocate();
            operations.push(Operation {
                id,
                result: psi_terminal::OperationResult::Unit,
                kind,
            });
        }
        if next_literal_argument != literal_places.len() {
            return unsupported("byte-sequence literal argument consumption is incomplete");
        }
        next_operation = operations.next_identity;
        let CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
            ..
        } = plan.operations.last().expect("Unit sequence was validated")
        else {
            unreachable!()
        };
        let trivial_affine_discards = trivial_affine_local_discard_ordinals
            .iter()
            .map(|ordinal| {
                local_places
                    .get(usize::try_from(*ordinal).map_err(|_| {
                        LoweringError::Unsupported("Unit local cleanup ordinal exceeds usize")
                    })?)
                    .map(|local| local.id)
                    .ok_or(LoweringError::Unsupported(
                        "Unit local cleanup ordinal is not dense",
                    ))
            })
            .chain(trivial_affine_discards.iter().map(|parameter_index| {
                parameters
                    .get(usize::try_from(*parameter_index).map_err(|_| {
                        LoweringError::Unsupported(
                            "Unit affine discard parameter index exceeds usize",
                        )
                    })?)
                    .map(|parameter| parameter.place)
                    .ok_or(LoweringError::Unsupported(
                        "Unit affine discard has an invalid parameter index",
                    ))
            }))
            .collect::<Result<Vec<_>, _>>()?;
        let block = block_id(allocate_dense(&mut next_block)?);
        let edge = edge_id(allocate_dense(&mut next_edge)?);
        let crash_routes =
            if let Some(contract_plan) = checked.facts.contract_plans.for_machine(plan.machine) {
                lower_structural_crash_route_buckets(
                    contract_plan.crash.published(),
                    parameters,
                    &structural_types,
                    runtime_requirements,
                )?
            } else {
                Vec::new()
            };
        machines.push(TerminalMachine {
            id: terminal_machine,
            attachment: Some(lookup_type_id(&type_ids, &plan.attachment_type_identity)?),
            parameters: Vec::new(),
            structural_parameters: parameters.clone(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: parameters
                .iter()
                .map(|parameter| StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::Parameter {
                        position: parameter.position,
                        is_self: parameter.is_self,
                    },
                })
                .chain(provider_places.iter().cloned())
                .chain(local_places.iter().cloned())
                .chain(literal_places.iter().cloned())
                .collect(),
            entry_claims: entry_claims.clone(),
            published_service_ceiling: if let Some(provider) = provider_candidate_plans
                .iter()
                .find(|candidate| candidate.candidate == plan.machine)
            {
                lower_provider_candidate_service_ceiling(
                    checked,
                    plans,
                    provider,
                    plan,
                    &service_ids,
                )?
            } else {
                lower_installation_machine_service_ceiling(
                    checked,
                    plan.machine,
                    plan.contract_service_reach,
                    plan.service_reach,
                    &service_ids,
                )?
            },
            content_entry_claims,
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: operations.operations,
                terminator: Terminator::ReturnUnit {
                    edge,
                    trivial_affine_discards,
                },
            }],
            contract: MachineContract {
                id: contract_id(terminal_machine.get()),
                crash_routes,
                requires: runtime_requirements.clone(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        });
    }

    let mut provider_candidates = provider_candidate_plans
        .iter()
        .map(|candidate| {
            let (_, boundary, parameters, scalar_parameters) = lowered_boundary_parameters
                .iter()
                .find(|(symbol, _, _, _)| *symbol == candidate.boundary)
                .ok_or(LoweringError::Unsupported(
                    "provider candidate references an unlowered Unit boundary requirement",
                ))?;
            if !scalar_parameters.is_empty() {
                return unsupported(
                    "provider candidate boundary signatures do not yet admit scalar parameters",
                );
            }
            let terminal_candidate = lookup_machine_id(&machine_ids, candidate.candidate)?;
            let realized = machines
                .iter()
                .find(|machine| machine.id == terminal_candidate)
                .expect("provider candidate root was lowered as an ordinary terminal machine");
            Ok(ProviderCandidateConformance {
                boundary: *boundary,
                requirement_identity: candidate.requirement_identity.clone(),
                provider_identity: candidate.provider_identity.clone(),
                candidate_identity: candidate.candidate_identity.clone(),
                candidate: terminal_candidate,
                signature: ProviderUnitSignature {
                    parameters: parameters
                        .iter()
                        .map(|parameter| ProviderSignatureParameter {
                            position: parameter.position,
                            is_self: parameter.is_self,
                            structural_type: parameter.structural_type,
                            multiplicity: parameter.multiplicity,
                            access: parameter.access,
                            qualifications: parameter.qualifications.clone(),
                        })
                        .collect(),
                },
                refinement: ProviderUnitRefinement {
                    positional_parameters: (0..parameters.len())
                        .map(|index| {
                            let index = u32::try_from(index).map_err(|_| {
                                LoweringError::Unsupported("provider signature arity exceeds u32")
                            })?;
                            Ok(ProviderParameterRefinement {
                                boundary_index: index,
                                candidate_index: index,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?,
                    required_domains: boundary_machines
                        .iter()
                        .find(|declaration| declaration.id == *boundary)
                        .expect("lowered provider boundary declaration exists")
                        .requires
                        .clone(),
                    realized_service_ceiling: realized.published_service_ceiling.clone(),
                },
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    provider_candidates.sort_by(|left, right| {
        (
            left.boundary,
            &left.provider_identity,
            &left.candidate_identity,
            left.candidate,
        )
            .cmp(&(
                right.boundary,
                &right.provider_identity,
                &right.candidate_identity,
                right.candidate,
            ))
    });

    Ok(LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine_id(1),
            structural_types,
            structural_domains,
            services,
            root_service_reach,
            boundary_machines,
            provider_candidates,
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines,
        },
        proof_bundle: ProofBundle {
            evidence_producers: Vec::new(),
            evidence: call_evidence,
        },
        debug_map: None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedUnitProviderCandidate {
    boundary: psi_symbols::SymbolHandle,
    candidate: psi_symbols::SymbolHandle,
    requirement_identity: String,
    provider_identity: String,
    candidate_identity: String,
}

//! Structural type, domain, and service catalog publication for attached Unit
//! closures.

use super::*;

pub(super) fn lower_program_local_root_introductions(
    checked: &CheckedTrees,
    plan: &CheckedBoundaryMachinePlan,
    requirement_identity: &str,
    parameters: &[StructuralParameterDeclaration],
    domain_ids: &[(SemanticDomainId, StructuralDomainId)],
) -> Result<Vec<ProgramLocalRootIntroductionSchema>, LoweringError> {
    let mut output = Vec::new();
    for requirement in &plan.domain_requirements {
        let Some(parameter) = plan
            .structural_parameters
            .get(requirement.argument_index as usize)
        else {
            return unsupported("program-local root route parameter is out of range");
        };
        let Some(projection) = checked
            .facts
            .qualifications
            .content
            .for_semantic_domain(requirement.domain)
        else {
            continue;
        };
        let Some(domain) = checked
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == projection.domain)
        else {
            // Synthetic and older checked fixtures may retain a qualification
            // row without the authored declaration that could authorize root
            // introduction. Preserve the ordinary requirement, but emit no
            // producer schema and therefore no authority-bearing candidate.
            continue;
        };
        let authorizes_requirement = domain.establishment_routes.iter().any(|route| {
            matches!(
                route,
                psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                    requirement,
                    ..
                } if *requirement == plan.machine
            )
        });
        if !authorizes_requirement {
            continue;
        }
        if projection.report_fingerprint == 0 {
            return unsupported("program-local root projection has a null identity");
        }
        let qualification = lookup_domain_id(domain_ids, requirement.domain)?;
        let qualification_identity = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_domains
            .iter()
            .find(|candidate| candidate.domain == requirement.domain)
            .map(|candidate| candidate.identity.as_str())
            .ok_or(LoweringError::Unsupported(
                "program-local root qualification has no normalized identity",
            ))?;
        let carrier = parameters
            .get(requirement.argument_index as usize)
            .ok_or(LoweringError::Unsupported(
                "program-local root terminal parameter is out of range",
            ))?
            .structural_type;
        let owner_projection = content_conservation::lower_structural_content_projection(
            checked,
            requirement.domain,
            &parameter.type_identity,
        )?
        .ok_or(LoweringError::Unsupported(
            "program-local root route has no owner content projection",
        ))?;
        let mut schema = ProgramLocalRootIntroductionSchema {
            argument_index: requirement.argument_index,
            source_parameter_position: parameter.position,
            qualification,
            carrier,
            projection: owner_projection.identity,
            algebra: owner_projection.algebra,
            capacity: owner_projection.expression,
            compatibility_report_identity: 0,
        };
        schema.compatibility_report_identity =
            program_local_root_introduction_compatibility_report_identity(
                requirement_identity,
                qualification_identity,
                &parameter.type_identity,
                &schema,
            );
        output.push(schema);
    }
    output.sort_by_key(|schema| (schema.argument_index, schema.qualification));
    if output.windows(2).any(|pair| {
        (pair[0].argument_index, pair[0].qualification)
            == (pair[1].argument_index, pair[1].qualification)
    }) {
        return unsupported("program-local root introduction schema is duplicated");
    }
    Ok(output)
}

pub(super) fn lower_unit_structural_types(
    checked: &CheckedTrees,
    closure: &[psi_symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
) -> Result<
    (
        Vec<StructuralTypeDeclaration>,
        Vec<(String, StructuralTypeId)>,
    ),
    LoweringError,
> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut roots = Vec::new();
    for symbol in closure {
        let machine = unique_unit_machine(plans, *symbol)?;
        roots.extend(machine.attachment_type_identity.iter().cloned());
        roots.extend(
            machine
                .structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity.clone()),
        );
        for local in &machine.trivial_affine_locals {
            roots.push(local.type_identity.clone());
            if let Some(construction) = &local.construction {
                roots.push(construction.root_type_identity.clone());
            }
        }
        for operation in &machine.operations {
            match operation {
                CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal {
                    type_identity,
                    ..
                } => roots.push(type_identity.clone()),
                CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                    realization_machine,
                    realization_state,
                    ..
                } => {
                    let realizations = checked
                        .facts
                        .flow
                        .terminal_structural_scalar_returns
                        .machines
                        .iter()
                        .filter(|plan| {
                            plan.machine == *realization_machine && plan.state == *realization_state
                        })
                        .collect::<Vec<_>>();
                    let [realization] = realizations.as_slice() else {
                        return unsupported(
                            "selected structural-scalar Unit operation has no exact type catalog owner",
                        );
                    };
                    roots.push(realization.attachment_type_identity.clone());
                    roots.extend(
                        realization
                            .structural_parameters
                            .iter()
                            .map(|parameter| parameter.type_identity.clone()),
                    );
                }
                CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                    realization_machine,
                    realization_state,
                    ..
                } => {
                    let realizations = checked
                        .facts
                        .flow
                        .terminal_structural_returns
                        .claim_free_affine_machines
                        .iter()
                        .filter(|plan| {
                            plan.machine == *realization_machine && plan.state == *realization_state
                        })
                        .collect::<Vec<_>>();
                    let [realization] = realizations.as_slice() else {
                        return unsupported(
                            "selected structural-result Unit operation has no exact type catalog owner",
                        );
                    };
                    roots.push(realization.attachment_type_identity.clone());
                    roots.push(realization.structural_parameter.type_identity.clone());
                    roots.push(realization.result.type_identity.clone());
                }
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. } => {
                    roots.push(result.type_identity.clone());
                }
                _ => {}
            }
        }
    }
    for (boundary, _) in boundaries {
        roots.extend(boundary.attachment_type_identity.iter().cloned());
        roots.extend(
            boundary
                .structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity.clone()),
        );
        roots.extend(boundary.result.structural_identity().map(str::to_owned));
    }
    lower_unit_structural_type_roots(checked, &roots)
}

pub(crate) fn lower_unit_structural_type_roots(
    checked: &CheckedTrees,
    roots: &[String],
) -> Result<
    (
        Vec<StructuralTypeDeclaration>,
        Vec<(String, StructuralTypeId)>,
    ),
    LoweringError,
> {
    fn collect(
        plans: &psi_checked_trees::CheckedUnitEffectPlans,
        identity: &str,
        active: &mut Vec<String>,
        selected: &mut Vec<String>,
    ) -> Result<(), LoweringError> {
        if active.iter().any(|candidate| candidate == identity) {
            return unsupported("recursive structural type is outside the Unit terminal slice");
        }
        if selected.iter().any(|candidate| candidate == identity) {
            return Ok(());
        }
        let mut matches = plans
            .structural_types
            .iter()
            .filter(|plan| plan.identity == identity);
        let plan = matches.next().ok_or(LoweringError::Unsupported(
            "Unit closure references a missing structural type",
        ))?;
        if matches.next().is_some() || identity.is_empty() {
            return unsupported(
                "Unit closure contains a duplicate or empty structural type identity",
            );
        }
        active.push(identity.to_owned());
        match &plan.shape {
            CheckedUnitStructuralTypeShape::PrimitiveScalar(_) => {}
            CheckedUnitStructuralTypeShape::ByteSequence(_) => {}
            CheckedUnitStructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                        &field.field_type
                    {
                        collect(plans, type_identity, active, selected)?;
                    }
                }
            }
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                ..
            } => {
                collect(plans, element_type_identity, active, selected)?;
            }
            CheckedUnitStructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                        &field.field_type
                    {
                        collect(plans, type_identity, active, selected)?;
                    }
                }
            }
            CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                        &field.field_type
                    {
                        collect(plans, type_identity, active, selected)?;
                    }
                }
            }
        }
        active.pop();
        selected.push(identity.to_owned());
        Ok(())
    }

    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut selected = Vec::new();
    let mut active = Vec::new();
    for identity in roots {
        collect(plans, identity, &mut active, &mut selected)?;
    }
    selected.sort();
    selected.dedup();
    let type_ids = selected
        .iter()
        .enumerate()
        .map(|(index, identity)| Ok((identity.clone(), structural_type_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_field = 1_u64;
    let mut next_case = 1_u64;
    let mut declarations = Vec::with_capacity(selected.len());
    for identity in selected {
        let plan = plans
            .structural_types
            .iter()
            .find(|plan| plan.identity == identity)
            .expect("selected structural type was validated above");
        let shape = match &plan.shape {
            CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive) => {
                StructuralTypeShape::PrimitiveScalar(terminal_scalar_type(*primitive)?)
            }
            CheckedUnitStructuralTypeShape::ByteSequence(carrier) => {
                StructuralTypeShape::ByteSequence(terminal_byte_sequence_carrier(*carrier))
            }
            CheckedUnitStructuralTypeShape::Record { fields } => {
                let mut field_identities = BTreeSet::new();
                let fields = fields.iter().map(|field| {
                if field.identity.is_empty() || !field_identities.insert(field.identity.as_str()) {
                    return Err(LoweringError::Unsupported(
                        "Unit structural type contains an empty or duplicate field identity",
                    ));
                }
                let (relevance, field_type) = match &field.field_type {
                    CheckedUnitStructuralFieldType::Scalar(primitive) => {
                        (field.relevance, terminal_structural_field_type(*primitive)?)
                    }
                    CheckedUnitStructuralFieldType::ByteSequence(carrier) => {
                        (field.relevance, StructuralFieldType::ByteSequence(terminal_byte_sequence_carrier(*carrier)))
                    }
                    CheckedUnitStructuralFieldType::Structural { type_identity } => {
                        (field.relevance, StructuralFieldType::Structural(lookup_type_id(&type_ids, type_identity)?))
                    }
                    CheckedUnitStructuralFieldType::ProviderBacked {
                        provider_type_identity,
                    } => (field.relevance, StructuralFieldType::Erased {
                        type_identity: provider_type_identity.clone(),
                    }),
                    CheckedUnitStructuralFieldType::FusedServiceBacked {
                        provider_type_identity,
                        erasure,
                    } => {
                        if !erasure.requirement.is_valid()
                            || erasure.provider_plan_digest == [0; 32]
                        {
                            return unsupported(
                                "fused Service erasure lacks an exact requirement or selected-provider-plan receipt",
                            );
                        }
                        (field.relevance, StructuralFieldType::Erased {
                            type_identity: provider_type_identity.clone(),
                        })
                    }
                    CheckedUnitStructuralFieldType::Erased { type_identity } => {
                        (field.relevance, StructuralFieldType::Erased {
                            type_identity: type_identity.clone(),
                        })
                    }
                };
                Ok(StructuralFieldDeclaration {
                    id: structural_field_id(allocate_dense(&mut next_field)?),
                    identity: field.identity.clone(),
                    relevance,
                    field_type,
                })
                }).collect::<Result<Vec<_>, LoweringError>>()?;
                StructuralTypeShape::Record { fields }
            }
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                length,
            } => StructuralTypeShape::FixedArray {
                element: lookup_type_id(&type_ids, element_type_identity)?,
                length: *length,
            },
            CheckedUnitStructuralTypeShape::Sum { cases } => {
                let mut case_identities = BTreeSet::new();
                let cases = cases
                    .iter()
                    .map(|case| {
                        if case.identity.is_empty()
                            || !case_identities.insert(case.identity.as_str())
                        {
                            return Err(LoweringError::Unsupported(
                                "Unit structural type contains an empty or duplicate case identity",
                            ));
                        }
                        Ok(StructuralCaseDeclaration {
                            id: StructuralCaseId::new(allocate_dense(&mut next_case)?)
                                .expect("allocated structural case identity is nonzero"),
                            identity: case.identity.clone(),
                            fields: {
                                let mut field_identities = BTreeSet::new();
                                case.fields
                                    .iter()
                                    .map(|field| {
                                        if field.identity.is_empty()
                                            || !field_identities.insert(field.identity.as_str())
                                        {
                                            return Err(LoweringError::Unsupported(
                                                "Unit structural sum case contains an empty or duplicate payload field identity",
                                            ));
                                        }
                                        let field_type = match &field.field_type {
                                            CheckedUnitStructuralFieldType::Scalar(primitive) => {
                                                terminal_structural_field_type(*primitive)?
                                            }
                                            CheckedUnitStructuralFieldType::ByteSequence(
                                                carrier,
                                            ) => StructuralFieldType::ByteSequence(
                                                terminal_byte_sequence_carrier(*carrier),
                                            ),
                                            CheckedUnitStructuralFieldType::Structural {
                                                type_identity,
                                            } => StructuralFieldType::Structural(lookup_type_id(
                                                &type_ids,
                                                type_identity,
                                            )?),
                                            CheckedUnitStructuralFieldType::ProviderBacked { .. }
                                            | CheckedUnitStructuralFieldType::FusedServiceBacked { .. } => {
                                                return unsupported("provider-backed attachment fields are valid only on records");
                                            }
                                            CheckedUnitStructuralFieldType::Erased {
                                                type_identity,
                                            } => StructuralFieldType::Erased {
                                                type_identity: type_identity.clone(),
                                            },
                                        };
                                        Ok(StructuralFieldDeclaration {
                                            id: structural_field_id(allocate_dense(
                                                &mut next_field,
                                            )?),
                                            identity: field.identity.clone(),
                                            relevance: field.relevance,
                                            field_type,
                                        })
                                    })
                                    .collect::<Result<Vec<_>, LoweringError>>()?
                            },
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                StructuralTypeShape::Sum { cases }
            }
            CheckedUnitStructuralTypeShape::Mixed { fields, cases } => StructuralTypeShape::Mixed {
                fields: lower_mixed_fields(fields, &type_ids, &mut next_field)?,
                cases: lower_mixed_cases(cases, &type_ids, &mut next_field, &mut next_case)?,
            },
        };
        declarations.push(StructuralTypeDeclaration {
            id: lookup_type_id(&type_ids, &identity)?,
            identity,
            shape,
        });
    }
    Ok((declarations, type_ids))
}

pub(super) fn lower_unit_structural_domains(
    checked: &CheckedTrees,
    closure: &[psi_symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    type_ids: &[(String, StructuralTypeId)],
) -> Result<
    (
        Vec<StructuralDomainDeclaration>,
        Vec<(psi_language_semantics::SemanticDomainId, StructuralDomainId)>,
    ),
    LoweringError,
> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut selected = Vec::new();
    for symbol in closure {
        let machine = unique_unit_machine(plans, *symbol)?;
        for domain in machine
            .structural_parameters
            .iter()
            .flat_map(|parameter| &parameter.qualifications)
            .chain(&machine.body_qualifications)
        {
            if !selected.contains(domain) {
                selected.push(*domain);
            }
        }
    }
    for (boundary, _) in boundaries {
        for domain in boundary
            .structural_parameters
            .iter()
            .flat_map(|parameter| &parameter.qualifications)
            .chain(
                boundary
                    .domain_requirements
                    .iter()
                    .map(|requirement| &requirement.domain),
            )
        {
            if !selected.contains(domain) {
                selected.push(*domain);
            }
        }
    }
    let mut selected_plans = selected
        .into_iter()
        .map(|domain| {
            let mut matches = plans
                .structural_domains
                .iter()
                .filter(|plan| plan.domain == domain);
            let plan = matches.next().ok_or(LoweringError::Unsupported(
                "Unit closure references a missing structural domain",
            ))?;
            if matches.next().is_some()
                || !domain.is_valid()
                || plan.identity.is_empty()
                || plan.carrier_type_identity.is_empty()
            {
                return Err(LoweringError::Unsupported(
                    "Unit structural domain is duplicate, null, or incomplete",
                ));
            }
            Ok(plan)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    selected_plans.sort_by(|left, right| left.identity.cmp(&right.identity));
    if selected_plans
        .windows(2)
        .any(|pair| pair[0].identity == pair[1].identity)
    {
        return unsupported("Unit structural domains have duplicate canonical identities");
    }
    let domain_ids = selected_plans
        .iter()
        .enumerate()
        .map(|(index, plan)| Ok((plan.domain, structural_domain_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let declarations = selected_plans
        .into_iter()
        .map(|plan| {
            let content_projection = content_conservation::lower_structural_content_projection(
                checked,
                plan.domain,
                &plan.carrier_type_identity,
            )?;
            Ok(StructuralDomainDeclaration {
                id: lookup_domain_id(&domain_ids, plan.domain)?,
                semantic_domain: DomainSemanticId::new(u64::from(plan.domain.0))
                    .ok_or(LoweringError::InvalidContentDomainIdentity)?,
                identity: plan.identity.clone(),
                carrier: lookup_type_id(type_ids, &plan.carrier_type_identity)?,
                content_projection,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok((declarations, domain_ids))
}

pub(super) fn lower_unit_services(
    checked: &CheckedTrees,
    closure: &[psi_symbols::SymbolHandle],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    provider_candidates: &[CheckedUnitProviderCandidate],
) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError> {
    let facts = &checked.facts.service_reaches;
    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut selected = Vec::<ServiceReachId>::new();
    for symbol in closure {
        let machine = unique_unit_machine(plans, *symbol)?;
        if let Some(provider) = provider_candidates
            .iter()
            .find(|candidate| candidate.candidate == *symbol)
        {
            collect_provider_candidate_services(
                &facts.rows,
                plans,
                provider,
                machine,
                &mut selected,
            )?;
        } else {
            collect_installation_machine_contract_services(
                checked,
                *symbol,
                machine.contract_service_reach,
                machine.service_reach,
                &mut selected,
            )?;
        }
        for operation in &machine.operations {
            match operation {
                CheckedUnitEffectOperationPlan::CallUnit { service_reach, .. }
                | CheckedUnitEffectOperationPlan::ScalarCall { service_reach, .. }
                | CheckedUnitEffectOperationPlan::BoundaryCall { service_reach, .. }
                | CheckedUnitEffectOperationPlan::BoundaryScalarCall { service_reach, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    service_reach, ..
                }
                | CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                    service_reach,
                    ..
                }
                | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                    service_reach,
                    ..
                }
                | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                    service_reach,
                    ..
                }
                | CheckedUnitEffectOperationPlan::PortWrite { service_reach, .. } => {
                    collect_service_summary(&facts.rows, *service_reach, &mut selected)?;
                }
                CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd { .. }
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                | CheckedUnitEffectOperationPlan::ReturnUnit { .. } => {}
            }
        }
    }
    for (boundary, _) in boundaries {
        collect_published_contract_services(
            &facts.rows,
            boundary.contract_service_reach,
            boundary.service_reach,
            &mut selected,
        )?;
    }
    lower_selected_unit_services(checked, selected)
}

pub(super) fn lower_selected_unit_services(
    checked: &CheckedTrees,
    mut selected: Vec<ServiceReachId>,
) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError> {
    let facts = &checked.facts.service_reaches;
    let mut next = 0_usize;
    while let Some(service) = selected.get(next).copied() {
        next += 1;
        let definition = facts
            .services
            .definition(service)
            .ok_or(LoweringError::Unsupported(
                "Unit closure references an unknown checked service",
            ))?;
        for parent in &definition.parents {
            if !selected.contains(parent) {
                selected.push(*parent);
            }
        }
    }
    let mut selected_definitions = selected
        .iter()
        .map(|service| {
            facts
                .services
                .definition(*service)
                .map(|definition| (*service, definition))
                .ok_or(LoweringError::Unsupported(
                    "Unit closure references an unknown checked service",
                ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    selected_definitions.sort_by(|left, right| left.1.name.cmp(&right.1.name));
    if selected_definitions
        .iter()
        .any(|(_, definition)| definition.name.is_empty())
        || selected_definitions
            .windows(2)
            .any(|pair| pair[0].1.name == pair[1].1.name)
    {
        return unsupported("Unit services have empty or duplicate canonical identities");
    }
    let service_ids = selected_definitions
        .iter()
        .enumerate()
        .map(|(index, (source, _))| Ok((*source, service_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let declarations = selected_definitions
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

pub(crate) fn lower_root_service_reach(
    checked: &CheckedTrees,
    entry: psi_symbols::SymbolHandle,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<psi_terminal::TerminalRootServiceReach, LoweringError> {
    let Some(reach) = checked.facts.service_reaches.for_machine(entry) else {
        return Ok(psi_terminal::TerminalRootServiceReach::default());
    };
    let mut concrete = checked
        .facts
        .service_reaches
        .rows
        .services(reach.concrete_effective)
        .iter()
        .map(|service| lookup_service_id(service_ids, *service))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    concrete.sort();
    concrete.dedup();

    let mut installation_dependencies = reach
        .unresolved_installation_reaches
        .iter()
        .map(|dependency| {
            let trait_matches = checked
                .typed
                .traits()
                .iter()
                .flat_map(|owner| {
                    checked
                        .typed
                        .trait_machine_signatures(owner)
                        .iter()
                        .filter(move |requirement| requirement.symbol == dependency.requirement)
                        .map(move |requirement| (owner, requirement))
                })
                .collect::<Vec<_>>();
            let machine_matches = checked
                .typed
                .machines()
                .iter()
                .filter(|machine| machine.symbol == dependency.requirement)
                .collect::<Vec<_>>();
            let requirement_identity = match (trait_matches.as_slice(), machine_matches.as_slice())
            {
                ([(owner, requirement)], []) => checked
                    .typed
                    .normalized_trait_requirement_overload_identity(owner, requirement)
                    .identity(),
                ([], [machine])
                    if machine.service_reach_is_installation_bound
                        && matches!(
                            machine.supply_mode,
                            psi_language_semantics::MachineSupplyMode::TopLevelRequirement
                                | psi_language_semantics::MachineSupplyMode::Boundary
                        ) =>
                {
                    checked
                        .typed
                        .normalized_machine_overload_identity(machine)
                        .ok_or({
                            LoweringError::Unsupported(
                                "top-level installation reach requirement has no normalized machine overload identity",
                            )
                        })?
                        .identity()
                }
                _ => {
                    return unsupported(
                        "terminal installation reach does not resolve to one exact typed requirement",
                    );
                }
            };
            let mut upper_bound = checked
                .facts
                .service_reaches
                .rows
                .services(dependency.upper_bound)
                .iter()
                .map(|service| lookup_service_id(service_ids, *service))
                .collect::<Result<Vec<_>, LoweringError>>()?;
            upper_bound.sort();
            upper_bound.dedup();
            Ok(InstallationReachDependency {
                requirement_identity,
                upper_bound,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    installation_dependencies
        .sort_by(|left, right| left.requirement_identity.cmp(&right.requirement_identity));
    if installation_dependencies
        .windows(2)
        .any(|pair| pair[0].requirement_identity == pair[1].requirement_identity)
    {
        return unsupported("terminal installation reach contains duplicate requirements");
    }
    Ok(psi_terminal::TerminalRootServiceReach {
        concrete,
        installation_dependencies,
    })
}

pub(crate) fn collect_contract_services(
    rows: &psi_language_semantics::ServiceReachRowTable,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    selected: &mut Vec<ServiceReachId>,
) -> Result<(), LoweringError> {
    collect_service_summary(rows, summary, selected)?;
    if contract.checked_inferred != summary.transitive {
        return unsupported(
            "Unit contract reach does not match the exact checked transitive reach",
        );
    }
    let ServiceReachInterface::PublishedCeiling(published) = contract.interface else {
        return Ok(());
    };
    require_valid_service_row(published)?;
    let ceiling = rows.services(published);
    if rows
        .services(summary.transitive)
        .iter()
        .any(|service| !ceiling.contains(service))
    {
        return unsupported("checked Unit service reach exceeds its published ceiling");
    }
    for service in ceiling {
        if !selected.contains(service) {
            selected.push(*service);
        }
    }
    Ok(())
}

pub(crate) fn collect_published_contract_services(
    rows: &psi_language_semantics::ServiceReachRowTable,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    selected: &mut Vec<ServiceReachId>,
) -> Result<(), LoweringError> {
    if matches!(contract.interface, ServiceReachInterface::InternalInferred) {
        return unsupported("public Unit contract has no published service ceiling");
    }
    collect_contract_services(rows, contract, summary, selected)
}

pub(crate) fn collect_installation_machine_contract_services(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    selected: &mut Vec<ServiceReachId>,
) -> Result<(), LoweringError> {
    if let Some(reach) = checked.facts.service_reaches.for_machine(machine)
        && matches!(contract.interface, ServiceReachInterface::InternalInferred)
        && !reach.unresolved_installation_reaches.is_empty()
    {
        if contract.checked_inferred != summary.transitive
            || reach.inferred_transitive != summary.transitive
        {
            return unsupported(
                "installation-bound machine reach disagrees with its checked transitive row",
            );
        }
        return collect_service_summary(&checked.facts.service_reaches.rows, summary, selected);
    }
    collect_contract_services(
        &checked.facts.service_reaches.rows,
        contract,
        summary,
        selected,
    )
}

fn collect_provider_candidate_services(
    rows: &psi_language_semantics::ServiceReachRowTable,
    plans: &psi_checked_trees::CheckedUnitEffectPlans,
    provider: &CheckedUnitProviderCandidate,
    candidate: &CheckedUnitEffectMachinePlan,
    selected: &mut Vec<ServiceReachId>,
) -> Result<(), LoweringError> {
    collect_service_summary(rows, candidate.service_reach, selected)?;
    if candidate.contract_service_reach.checked_inferred != candidate.service_reach.transitive {
        return unsupported(
            "checked provider adapter contract reach does not match its transitive reach",
        );
    }
    let boundary = unique_unit_boundary(plans, provider.boundary)?;
    let ceiling = match boundary.contract_service_reach.interface {
        ServiceReachInterface::PublishedCeiling(row) => row,
        ServiceReachInterface::InternalInferred => {
            return unsupported("checked provider boundary has no published service ceiling");
        }
    };
    if rows
        .services(candidate.service_reach.transitive)
        .iter()
        .any(|service| !rows.services(ceiling).contains(service))
    {
        return unsupported("checked provider adapter reach exceeds its boundary requirement");
    }
    Ok(())
}

pub(super) fn lower_provider_candidate_service_ceiling(
    checked: &CheckedTrees,
    plans: &psi_checked_trees::CheckedUnitEffectPlans,
    provider: &CheckedUnitProviderCandidate,
    candidate: &CheckedUnitEffectMachinePlan,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    let rows = &checked.facts.service_reaches.rows;
    let mut selected = Vec::new();
    collect_provider_candidate_services(rows, plans, provider, candidate, &mut selected)?;
    let source = rows.services(candidate.service_reach.transitive);
    let mut lowered = source
        .iter()
        .map(|service| lookup_service_id(service_ids, *service))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    lowered.sort();
    lowered.dedup();
    if lowered.len() != source.len() {
        return unsupported("checked provider adapter reach contains duplicates");
    }
    Ok(lowered)
}

pub(crate) fn checked_unit_target_reach_matches(
    call: ServiceReachSummary,
    target_contract: ServiceReachPlan,
) -> bool {
    let expected = match target_contract.interface {
        ServiceReachInterface::PublishedCeiling(row) => row,
        ServiceReachInterface::InternalInferred => target_contract.checked_inferred,
    };
    call.transitive == expected
}

pub(crate) fn collect_service_summary(
    rows: &psi_language_semantics::ServiceReachRowTable,
    summary: ServiceReachSummary,
    selected: &mut Vec<ServiceReachId>,
) -> Result<(), LoweringError> {
    require_valid_service_row(summary.direct)?;
    require_valid_service_row(summary.transitive)?;
    let transitive = rows.services(summary.transitive);
    if rows
        .services(summary.direct)
        .iter()
        .any(|service| !transitive.contains(service))
    {
        return unsupported("Unit direct service reach is not contained in transitive reach");
    }
    for service in rows.services(summary.direct).iter().chain(transitive) {
        if !selected.contains(service) {
            selected.push(*service);
        }
    }
    Ok(())
}

pub(super) fn require_valid_service_row(row: ServiceReachRowId) -> Result<(), LoweringError> {
    if row.is_valid() {
        Ok(())
    } else {
        unsupported("Unit closure contains a null checked service row")
    }
}

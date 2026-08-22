//! Structural type, domain, and service catalog publication for attached Unit
//! closures.

use super::*;

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
            CheckedUnitStructuralTypeShape::Sum { .. } => {}
        }
        active.pop();
        selected.push(identity.to_owned());
        Ok(())
    }

    let plans = &checked.facts.flow.terminal_unit_effects;
    let mut selected = Vec::new();
    let mut active = Vec::new();
    for symbol in closure {
        let machine = unique_unit_machine(plans, *symbol)?;
        collect(
            plans,
            &machine.attachment_type_identity,
            &mut active,
            &mut selected,
        )?;
        for parameter in &machine.structural_parameters {
            collect(plans, &parameter.type_identity, &mut active, &mut selected)?;
        }
        for local in &machine.trivial_affine_locals {
            collect(plans, &local.type_identity, &mut active, &mut selected)?;
        }
    }
    for (boundary, _) in boundaries {
        if let Some(identity) = &boundary.attachment_type_identity {
            collect(plans, identity, &mut active, &mut selected)?;
        }
        for parameter in &boundary.structural_parameters {
            collect(plans, &parameter.type_identity, &mut active, &mut selected)?;
        }
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
                    } => {
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
                                            CheckedUnitStructuralFieldType::ProviderBacked { .. } => {
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
            Ok(StructuralDomainDeclaration {
                id: lookup_domain_id(&domain_ids, plan.domain)?,
                identity: plan.identity.clone(),
                carrier: lookup_type_id(type_ids, &plan.carrier_type_identity)?,
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
            collect_contract_services(
                &facts.rows,
                machine.contract_service_reach,
                machine.service_reach,
                &mut selected,
            )?;
        }
        for operation in &machine.operations {
            match operation {
                CheckedUnitEffectOperationPlan::CallUnit { service_reach, .. }
                | CheckedUnitEffectOperationPlan::BoundaryCall { service_reach, .. }
                | CheckedUnitEffectOperationPlan::PortWrite { service_reach, .. } => {
                    collect_service_summary(&facts.rows, *service_reach, &mut selected)?;
                }
                CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                | CheckedUnitEffectOperationPlan::ReturnUnit { .. } => {}
            }
        }
    }
    for (boundary, _) in boundaries {
        collect_contract_services(
            &facts.rows,
            boundary.contract_service_reach,
            boundary.service_reach,
            &mut selected,
        )?;
    }
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
            let matches = checked
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
            let [(owner, requirement)] = matches.as_slice() else {
                return unsupported(
                    "terminal installation reach does not resolve to one exact typed requirement",
                );
            };
            let requirement_identity = checked
                .typed
                .normalized_trait_requirement_overload_identity(owner, requirement)
                .identity();
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
    let published = match contract.interface {
        ServiceReachInterface::PublishedCeiling(row) => row,
        ServiceReachInterface::InternalInferred => {
            if rows.services(summary.transitive).is_empty() {
                return Ok(());
            }
            return unsupported("effectful Unit machine has no published service ceiling");
        }
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

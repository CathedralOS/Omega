//! Result-bearing boundary custody lowering.
use super::{
    BoundaryMachineDeclaration, BoundaryMachineResult, CheckedBoundaryMachinePlan,
    CheckedBoundaryScalarReturnMachinePlan, CheckedTrees, DomainSemanticId, LoweredPsi,
    LoweringError, ProofBundle, ServiceDeclaration, ServiceId, ServiceReachId, ServiceReachSummary,
    StructuralDomainDeclaration, StructuralDomainRequirement, StructuralTypeId, TerminalModule,
    boundary_machine_id, checked_unit_boundary_identity,
    collect_installation_machine_contract_services, collect_published_contract_services,
    collect_service_summary, content_conservation, contract_id, dense_identity,
    finalize_operation_proofs, lookup_service_id, lookup_type_id,
    lower_boundary_content_guarantees, lower_boundary_crash_routes,
    lower_published_service_ceiling, lower_root_service_reach, lower_structural_type_plans,
    lower_unit_parameters, machine_id, service_id, structural_domain_id, terminal_scalar_type,
    unsupported,
};
use crate::producer_result::SourceMappedLowered;
use crate::scalar_graph::scalar_call_closure::embedded::EmbeddedScalarCalls;
use crate::terminal_identities::{
    StructuralDomainBinding, lookup_domain_id_for_carrier, structural_domain_identity,
};
use crate::unit::attached_unit::argument_evaluation;

mod emission;
mod memberships;
mod requirements;
mod source_custody;
mod validation;

pub(crate) use emission::{
    BoundaryScalarReturnCatalogs, BoundaryScalarReturnIdentities, EmittedBoundaryScalarReturn,
    emit_boundary_scalar_return,
};
pub(crate) use requirements::checked_requirements;
pub(crate) use validation::validate_boundary_scalar_return;

/// The call a result-bearing boundary plan retains: a boundary call with a
/// Unit result. The machine's return reads the boundary's own scalar result.
pub(crate) fn retained_boundary_call<'plan>(
    plan: &'plan CheckedBoundaryScalarReturnMachinePlan,
    missing: &'static str,
) -> Result<&'plan checked_trees::CheckedUnitCallPlan, LoweringError> {
    plan.boundary_call
        .call()
        .filter(|call| {
            call.is_boundary() && *call.result() == checked_trees::CheckedUnitCallResult::Unit
        })
        .ok_or(LoweringError::Unsupported(missing))
}

pub(crate) fn lower_boundary_scalar_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedBoundaryScalarReturnMachinePlan,
) -> Result<SourceMappedLowered, LoweringError> {
    let boundary = validate_boundary_scalar_return(checked, plan)?;
    crate::unit::attached_unit::validate_requirement_kind(checked, boundary)?;
    let plans = &checked.facts.flow.terminal_boundary_scalar_returns;
    let call = retained_boundary_call(
        plan,
        "result-bearing boundary plan does not contain a boundary call",
    )?;
    let service_reach = &call.target.service_reach;
    let scalar_arguments = &call.scalar_arguments;
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
        &structural_types,
        &domain_ids,
        &mut next_place,
    )?;
    let boundary_parameters = lower_unit_parameters(
        &boundary.structural_parameters,
        &type_ids,
        &structural_types,
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
                domain: lookup_domain_id_for_carrier(
                    &domain_ids,
                    requirement.domain,
                    boundary_parameters
                        .get(requirement.argument_index as usize)
                        .ok_or(LoweringError::Unsupported(
                            "boundary structural requirement has an invalid argument index",
                        ))?
                        .structural_type,
                )?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    requires.sort();
    requires.dedup();
    let boundary_id = boundary_machine_id(1);
    let boundary_declaration = BoundaryMachineDeclaration {
        kind: boundary.kind,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        parameter_order: crate::unit::attached_unit::lower_boundary_parameter_order(
            &boundary.scalar_parameters,
            &boundary.structural_parameters,
        )?,
        id: boundary_id,
        identity: checked_unit_boundary_identity(checked, boundary.machine)?,
        attachment: boundary
            .attachment_type_identity
            .as_ref()
            .map(|identity| lookup_type_id(&type_ids, identity))
            .transpose()?,
        scalar_parameters: boundary_scalar_parameters.clone(),
        crash_routes: lower_boundary_crash_routes(checked, boundary, &boundary_scalar_parameters)?,
        structural_parameters: boundary_parameters,
        result: BoundaryMachineResult::Scalar(terminal_scalar_type(plan.result_type)?),
        requires,
        program_local_root_introductions: Vec::new(),
        content_guarantees: lower_boundary_content_guarantees(
            &checked.facts.qualifications.content.conservation_plans,
            boundary.state,
        )?,
        fixed_service_reach: crate::unit::attached_unit::lower_fixed_boundary_service_reach(
            checked,
            boundary,
            &service_ids,
        )?,
        published_service_ceiling: lower_published_service_ceiling(
            &checked.facts.service_reaches.rows,
            boundary.contract_service_reach,
            boundary.service_reach,
            &service_ids,
        )?,
    };

    let roots = scalar_arguments
        .iter()
        .filter_map(|argument| match argument {
            checked_trees::CheckedCallScalarArgument::Computation(root) => Some(*root),
            checked_trees::CheckedCallScalarArgument::Pure(_) => None,
        })
        .collect::<Vec<_>>();
    let scalar_calls =
        EmbeddedScalarCalls::prepare_computations(checked, &roots, &[plan.machine], 1)?;
    let mut source_machine_ids = vec![(plan.machine, machine_id(1))];
    source_machine_ids.extend_from_slice(&scalar_calls.machine_ids);
    let EmittedBoundaryScalarReturn {
        machine,
        source_call_occurrences,
        selected_ieee_float_comparison_occurrences,
        selected_integer_comparison_occurrences,
    } = emit_boundary_scalar_return(
        checked,
        plan,
        parameters,
        BoundaryScalarReturnCatalogs {
            structural_types: &structural_types,
            type_ids: &type_ids,
            service_ids: &service_ids,
        },
        BoundaryScalarReturnIdentities {
            machine: machine_id(1),
            contract: contract_id(1),
            boundary: boundary_id,
            identity_base: 0,
        },
        &mut scalar_calls.emission_context(),
    )?;
    let entry = machine.id;
    let mut lowered = LoweredPsi {
        semantic_module: TerminalModule {
            structural_types,
            structural_domains,
            services,
            root_service_reach,
            boundary_machines: vec![boundary_declaration],
            machines: vec![machine],
            ..TerminalModule::for_entry(entry)
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences,
        selected_ieee_float_comparison_occurrences,
        selected_integer_comparison_occurrences,
    };
    scalar_calls.append_to(&mut lowered)?;
    finalize_operation_proofs(&mut lowered)?;
    Ok(SourceMappedLowered {
        terminal: lowered,
        source_machine_ids,
    })
}

fn lower_boundary_scalar_domains(
    checked: &CheckedTrees,
    plans: &checked_trees::CheckedBoundaryScalarReturnPlans,
    machine: &CheckedBoundaryScalarReturnMachinePlan,
    boundary: &CheckedBoundaryMachinePlan,
    type_ids: &[(String, StructuralTypeId)],
) -> Result<
    (
        Vec<StructuralDomainDeclaration>,
        Vec<StructuralDomainBinding>,
    ),
    LoweringError,
> {
    let mut selected = machine
        .structural_parameters
        .iter()
        .chain(boundary.structural_parameters.iter())
        .flat_map(|parameter| {
            parameter
                .qualifications
                .iter()
                .map(|domain| (*domain, parameter.type_identity.as_str()))
        })
        .collect::<Vec<_>>();
    for requirement in &boundary.domain_requirements {
        let parameter = boundary
            .structural_parameters
            .get(requirement.argument_index as usize)
            .ok_or(LoweringError::Unsupported(
                "boundary domain requirement has no structural parameter",
            ))?;
        selected.push((requirement.domain, parameter.type_identity.as_str()));
    }
    selected.sort_by_key(|(domain, carrier)| (domain.0, *carrier));
    selected.dedup();
    let mut selected_plans = selected
        .iter()
        .map(|(domain, carrier)| {
            let mut matches = plans
                .structural_domains
                .iter()
                .filter(|plan| plan.domain == *domain && plan.carrier_type_identity == *carrier);
            let selected = matches.next().ok_or(LoweringError::Unsupported(
                "result-bearing boundary references a missing structural domain",
            ))?;
            if matches.next().is_some() {
                return unsupported("result-bearing boundary repeats a structural domain carrier");
            }
            Ok(selected)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    selected_plans.sort_by(|left, right| {
        (&left.identity, &left.carrier_type_identity)
            .cmp(&(&right.identity, &right.carrier_type_identity))
    });
    if selected_plans.windows(2).any(|pair| {
        pair[0].identity == pair[1].identity
            && pair[0].carrier_type_identity == pair[1].carrier_type_identity
    }) {
        return unsupported("result-bearing boundary has duplicate structural domains");
    }
    let domain_ids = selected_plans
        .iter()
        .enumerate()
        .map(|(index, plan)| {
            Ok(StructuralDomainBinding {
                semantic_domain: plan.domain,
                carrier: lookup_type_id(type_ids, &plan.carrier_type_identity)?,
                domain: structural_domain_id(dense_identity(index)?),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let declarations = selected_plans
        .into_iter()
        .map(|plan| {
            Ok(StructuralDomainDeclaration {
                byte_predicate: plan.byte_predicate,
                establishment_routes: Vec::new(),
                id: lookup_domain_id_for_carrier(
                    &domain_ids,
                    plan.domain,
                    lookup_type_id(type_ids, &plan.carrier_type_identity)?,
                )?,
                semantic_domain: DomainSemanticId::new(u64::from(plan.domain.0))
                    .ok_or(LoweringError::InvalidContentDomainIdentity)?,
                identity: structural_domain_identity(&plan.identity, &plan.carrier_type_identity),
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

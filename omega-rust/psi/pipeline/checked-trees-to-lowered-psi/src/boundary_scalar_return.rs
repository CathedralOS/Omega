//! Result-bearing boundary custody lowering.

use super::*;
use crate::attached_unit::argument_evaluation;
use crate::machine_dispatch::SourceMappedLowered;
use crate::scalar_call_closure::embedded::EmbeddedScalarCalls;

mod emission;
mod source_custody;
mod validation;

pub(crate) use emission::{
    BoundaryScalarReturnCatalogs, BoundaryScalarReturnIdentities, EmittedBoundaryScalarReturn,
    emit_boundary_scalar_return,
};
pub(crate) use validation::validate_boundary_scalar_return;

pub(super) fn lower_boundary_scalar_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedBoundaryScalarReturnMachinePlan,
) -> Result<SourceMappedLowered, LoweringError> {
    let boundary = validate_boundary_scalar_return(checked, plan)?;
    let plans = &checked.facts.flow.terminal_boundary_scalar_returns;
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        service_reach,
        scalar_arguments,
        ..
    } = &plan.boundary_call
    else {
        return unsupported("result-bearing boundary plan does not contain a boundary call");
    };
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
        result: BoundaryMachineResult::Scalar(terminal_scalar_type(plan.result_type)?),
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
        selected_ieee_float_fma_occurrences,
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
    let mut lowered = LoweredPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine.id,
            structural_types,
            structural_domains,
            services,
            root_service_reach,
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: vec![boundary_declaration],
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
            machines: vec![machine],
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences,
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

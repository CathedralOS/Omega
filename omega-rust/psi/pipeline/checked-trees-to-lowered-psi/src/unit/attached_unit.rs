//! Attached Unit closure assembly.
//!
//! The orchestrator retains exact transitive closure and publication order;
//! `call_catalog` closes operation/scalar/provider dependencies before allocation;
//! `admission` retains checked bodies and validates source/call custody;
//! `signatures` allocates each machine's formals, claims and requirements.
//! Emission borrows those records in the single shared namespace.
use super::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, ClaimTransfer, CompletionReceipt, ContractClause, LoweredPsi,
    LoweringError, MachineContract, Multiplicity, Operation, OperationKind, OperationResult,
    PlaceId, ProofBundle, ProviderCandidateConformance, ProviderParameterRefinement,
    ProviderRefinement, ProviderSignature, ProviderSignatureParameter, ScalarType,
    SemanticDomainId, ServiceReachSummary, StructuralDomainId, StructuralDomainRequirement,
    StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, StructuralTypeShape, TERMINAL_MACHINE_IDENTITY_STRIDE,
    TERMINAL_UNIT_CALL_OBLIGATION_BASE, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker, allocate_dense, boundary_machine_id,
    content_conservation, contract_id, dense_identity, direct_expression_contains_short_circuit,
    edge_id, emit_direct_expression, finalize_operation_proofs, lookup_claim_id, lookup_domain_id,
    lookup_machine_id, lookup_service_id, lookup_type_id, lower_boundary_content_guarantees,
    lower_boundary_crash_routes, lower_checked_crash_route_buckets,
    lower_checked_scalar_expression, lower_placed_view_input, lower_structural_crash_route_buckets,
    machine_id, obligation_id, place_id, terminal_scalar_type, unsupported,
    validate_direct_parameter_types, value_id,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::expression_preparation::source_custody::flow_calls::retain_exact_flow_call;
use crate::scalar_graph::scalar_call_closure::callee::{CheckedScalarCallee, PreparedScalarCallee};
use checked_trees::CheckedUnitStructuralArgumentSourcePlan;

mod admission;
pub(crate) mod argument_evaluation;
mod argument_schedule;
pub(crate) mod bodies;
mod byte_subslices;
mod call_catalog;
mod call_closure;
pub(crate) mod catalog;
mod claims;
mod composed_control;
mod ordinary_calls;
mod parameters;
pub(crate) mod primitive_locals;
mod provider_attachments;
mod providers;
mod reference_results;
pub(crate) mod scalar_arrays;
mod scalar_boundaries;
mod scalar_completion;
mod scalar_structural_calls;
mod selected_operator;
pub(crate) mod shared_closure;
mod signatures;
mod structural_calls;
mod structural_completion;
pub(crate) mod structural_values;

use bodies::UnitBody;
pub(crate) use parameters::validate_direct_unit_parameter_custody;
pub(crate) use parameters::{lower_declared_service_reach, lower_fixed_boundary_service_reach};

use call_closure::{
    checked_scalar_call_closure_with_structural_roots, checked_terminal_machine_name,
    reject_recursive_unit_closure, unique_unit_boundary, validate_unit_operation_sequence,
};
pub(crate) use call_closure::{
    checked_unit_boundary_identity, checked_unit_call_closure_including, unique_unit_machine,
};
#[cfg(test)]
pub(crate) use catalog::collect_contract_services;
pub(crate) use catalog::{
    checked_unit_target_reach_matches, collect_installation_machine_contract_services,
    collect_published_contract_services, collect_service_summary, lower_root_service_reach,
    lower_unit_structural_type_roots,
};
use catalog::{
    lower_program_local_root_introductions, lower_provider_candidate_service_ceiling,
    require_valid_service_row,
};
pub(crate) use composed_control::dynamic_result::{
    emit_call_leaf as emit_dynamic_control_leaf,
    lower_control_catalogs as lower_dynamic_control_catalogs,
};
pub(crate) use composed_control::lower_composed_unit_control_machine;
#[cfg(test)]
pub(crate) use parameters::lower_contract_service_ceiling;
pub(crate) use parameters::{
    checked_scalar_source_parameters, literal_argument_places,
    lower_installation_machine_service_ceiling, lower_published_service_ceiling,
    lower_structural_arguments, lower_unit_parameters, structural_carrier_type,
    validate_transfer_shape,
};
pub(crate) use provider_attachments::lower_provider_attachment_places;
use provider_attachments::validate_provider_attachment_requirements;
use providers::{ProviderBody, checked_unit_provider_candidates};
use selected_operator::lower_selected_structural_scalar_realizations;

fn lower_boundary_result(
    result: &CheckedBoundaryMachineResultPlan,
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[(SemanticDomainId, StructuralDomainId)],
) -> Result<BoundaryMachineResult, LoweringError> {
    Ok(match result {
        CheckedBoundaryMachineResultPlan::Unit => BoundaryMachineResult::Unit,
        CheckedBoundaryMachineResultPlan::Scalar(scalar) => {
            BoundaryMachineResult::Scalar(terminal_scalar_type(*scalar)?)
        }
        CheckedBoundaryMachineResultPlan::Structural {
            type_identity,
            multiplicity,
            qualifications,
        } => BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
            structural_type: lookup_type_id(type_ids, type_identity)?,
            multiplicity: match multiplicity {
                Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                Multiplicity::Affine => StructuralMultiplicity::Affine,
                Multiplicity::Linear => StructuralMultiplicity::Linear,
            },
            qualifications: qualifications
                .iter()
                .map(|domain| lookup_domain_id(domain_ids, *domain))
                .collect::<Result<Vec<_>, _>>()?,
        }),
    })
}

fn retain_exact_checked_flow_call(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    target: symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    retain_exact_flow_call(checked, machine.machine, machine.state, coordinate, target).map(|_| ())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn retain_exact_unit_boundary<'plans>(
    checked: &CheckedTrees,
    plans: &'plans checked_trees::CheckedUnitEffectPlans,
    boundaries: &mut Vec<(&'plans CheckedBoundaryMachinePlan, String)>,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    target_contract_report_fingerprint: u64,
    service_reach: ServiceReachSummary,
    expected_result: CheckedBoundaryMachineResultPlan,
) -> Result<(), LoweringError> {
    let target = unique_unit_boundary(plans, target_machine)?;
    if target.contract_report_fingerprint == 0 {
        return unsupported("Unit boundary target has a null checked contract fingerprint");
    }
    if target.state != target_state
        || target.contract_report_fingerprint != target_contract_report_fingerprint
        || target.result != expected_result
        || !checked_unit_target_reach_matches(service_reach, target.contract_service_reach)
    {
        return unsupported(
            "Unit boundary call does not match the exact checked target state, result, contract, and reach",
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
    Ok(())
}

pub(crate) fn lower_unit_effect_closure(
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    let closure = lower_shared_unit_closure(checked, entry, &[entry], None)?;
    crate::producer_result::SourceMappedLowered::new(closure.lowered, closure.machine_ids)
}

/// Nominal cleanup assembles the final caller and cleanup contracts itself,
/// after restoring their full scalar and structural namespaces.
pub(crate) fn lower_nominal_cleanup_closure(
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
    additional_roots: &[symbols::SymbolHandle],
) -> Result<LoweredPsi, LoweringError> {
    let mut roots = vec![entry];
    roots.extend_from_slice(additional_roots);
    assemble_unit_closure(
        checked,
        entry,
        &roots,
        None,
        RuntimeRequirementOwner::NominalCleanup,
        false,
    )
    .map(|closure| closure.lowered)
}

pub(crate) fn lower_shared_unit_closure(
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
    unit_roots: &[symbols::SymbolHandle],
    external: Option<shared_closure::ExternalUnitRoots<'_>>,
) -> Result<shared_closure::SharedUnitClosure, LoweringError> {
    assemble_unit_closure(
        checked,
        entry,
        unit_roots,
        external,
        RuntimeRequirementOwner::UnitClosure,
        false,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeRequirementOwner {
    UnitClosure,
    NominalCleanup,
}

/// A scalar entry uses the same real callee catalog without a synthetic Unit caller.
pub(crate) fn lower_scalar_effect_closure(
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    let mut closure = assemble_unit_closure(
        checked,
        entry,
        &[],
        None,
        RuntimeRequirementOwner::UnitClosure,
        true,
    )?;
    finalize_operation_proofs(&mut closure.lowered)?;
    crate::producer_result::SourceMappedLowered::new(closure.lowered, closure.machine_ids)
}

fn assemble_unit_closure(
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
    unit_roots: &[symbols::SymbolHandle],
    external: Option<shared_closure::ExternalUnitRoots<'_>>,
    requirements_owner: RuntimeRequirementOwner,
    scalar_entry: bool,
) -> Result<shared_closure::SharedUnitClosure, LoweringError> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    let reserved_prefix = usize::from(external.is_some());
    let ordinary_entry = unit_roots.first().copied();
    if external.is_none() && !scalar_entry && ordinary_entry != Some(entry) {
        return unsupported("ordinary Unit closure must begin with its exact entry");
    }
    let call_catalog =
        call_catalog::discover(checked, entry, unit_roots, external.as_ref(), scalar_entry)?;
    let closure = call_catalog.operations;
    let provider_candidate_plans = call_catalog.providers;
    let scalar_closure = call_catalog.scalars;
    let scalar_callees = scalar_closure
        .iter()
        .map(|machine| {
            crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee::find_for_unit_call(
                checked, *machine,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut selected_structural_scalar_roots = Vec::new();
    for machine_symbol in &closure {
        for operation in UnitBody::find(plans, *machine_symbol)?.operations() {
            if let CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                realization_machine,
                ..
            } = operation
                && !selected_structural_scalar_roots.contains(realization_machine)
            {
                selected_structural_scalar_roots.push(*realization_machine);
            }
        }
    }
    if selected_structural_scalar_roots
        .iter()
        .any(|machine| closure.contains(machine) || scalar_closure.contains(machine))
    {
        return unsupported(
            "selected structural-scalar realization overlaps another attached Unit closure",
        );
    }
    let mut structural_result_roots = Vec::new();
    for candidate in provider_candidate_plans
        .iter()
        .filter(|candidate| candidate.body == ProviderBody::AffineIdentity)
    {
        if !structural_result_roots.contains(&candidate.candidate) {
            structural_result_roots.push(candidate.candidate);
        }
    }
    for machine_symbol in &closure {
        for operation in UnitBody::find(plans, *machine_symbol)?.operations() {
            let target = match operation {
                CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                    realization_machine,
                    ..
                } => Some(*realization_machine),
                CheckedUnitEffectOperationPlan::StructuralCall { target_machine, .. }
                    if !UnitBody::contains(plans, *target_machine) =>
                {
                    Some(*target_machine)
                }
                _ => None,
            };
            if let Some(target) = target
                && !structural_result_roots.contains(&target)
            {
                structural_result_roots.push(target);
            }
        }
    }
    if structural_result_roots.iter().any(|machine| {
        closure.contains(machine)
            || scalar_closure.contains(machine)
            || selected_structural_scalar_roots.contains(machine)
    }) {
        return unsupported("structural-result machine overlaps another attached Unit closure");
    }

    let mut boundaries = Vec::<(&CheckedBoundaryMachinePlan, String)>::new();
    if let Some(external) = &external {
        for symbol in external.boundary_roots {
            let boundary = unique_unit_boundary(plans, *symbol)?;
            retain_exact_unit_boundary(
                checked,
                plans,
                &mut boundaries,
                *symbol,
                boundary.state,
                boundary.contract_report_fingerprint,
                boundary.service_reach,
                boundary.result.clone(),
            )?;
        }
    }
    let admitted_bodies = admission::admit(
        checked,
        entry,
        &closure,
        &scalar_closure,
        requirements_owner,
        &mut boundaries,
    )?;
    let mut additional_type_roots = external
        .as_ref()
        .map_or_else(Vec::new, |roots| roots.structural_type_roots.to_vec());
    for candidate in provider_candidate_plans
        .iter()
        .filter(|candidate| candidate.body == ProviderBody::AffineIdentity)
    {
        let plan = providers::affine_candidate(checked, candidate.candidate)?;
        additional_type_roots.extend(plan.attachment_type_identity.iter().cloned());
        additional_type_roots.push(plan.structural_parameter.type_identity.clone());
        additional_type_roots.push(plan.result.type_identity.clone());
    }
    let mut additional_service_roots = external
        .as_ref()
        .map_or_else(Vec::new, |roots| roots.service_roots.to_vec());
    scalar_boundaries::retain_catalog_roots(
        checked,
        &scalar_callees,
        &mut boundaries,
        &mut additional_type_roots,
        &mut additional_service_roots,
    )?;
    boundaries.sort_by(|left, right| left.1.cmp(&right.1));
    if boundaries.windows(2).any(|pair| pair[0].1 == pair[1].1) {
        return unsupported("boundary Unit closure contains duplicate canonical identities");
    }

    let (mut structural_types, type_ids) = if !additional_type_roots.is_empty() {
        catalog::lower_unit_structural_types_including(
            checked,
            &closure,
            &boundaries,
            &additional_type_roots,
        )?
    } else {
        catalog::lower_unit_structural_types(checked, &closure, &boundaries)?
    };
    // Callable composed bodies borrow a complete shared catalog. Retain the
    // generated immutable carrier before cloning it into any callee emitter.
    for machine_symbol in &closure {
        if UnitBody::find(plans, *machine_symbol)?
            .operations()
            .any(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                )
            })
        {
            crate::emission::structural_byte_sequence_store::literal_view_type(
                &mut structural_types,
            )?;
            break;
        }
    }
    let wrapper_domains = scalar_callees
        .iter()
        .filter_map(|callee| match callee {
            CheckedScalarCallee::Boundary(plan) => Some(plan),
            _ => None,
        })
        .flat_map(|plan| &plan.structural_parameters)
        .flat_map(|parameter| {
            parameter.qualifications.iter().chain(
                parameter
                    .projected_qualifications
                    .iter()
                    .map(|row| &row.domain),
            )
        })
        .copied()
        .collect::<Vec<_>>();
    let (structural_domains, domain_ids) = catalog::lower_unit_structural_domains_including(
        checked,
        &closure,
        &boundaries,
        &type_ids,
        &wrapper_domains,
    )?;
    let (services, service_ids) = if !additional_service_roots.is_empty() {
        catalog::lower_unit_services_including(
            checked,
            &closure,
            &boundaries,
            &provider_candidate_plans,
            &additional_service_roots,
        )?
    } else {
        catalog::lower_unit_services(checked, &closure, &boundaries, &provider_candidate_plans)?
    };
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
                    .is_none_or(|index| index >= parameters.len())
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
            crash_routes: lower_boundary_crash_routes(checked, plan, &scalar_parameters)?,
            structural_parameters: parameters.clone(),
            result: lower_boundary_result(&plan.result, &type_ids, &domain_ids)?,
            requires,
            program_local_root_introductions,
            content_guarantees,
            fixed_service_reach: lower_fixed_boundary_service_reach(checked, plan, &service_ids)?,
            published_service_ceiling,
        });
        lowered_boundary_parameters.push((plan.machine, id, parameters, scalar_parameters));
    }

    let mut next_value = 1_u64;
    let machine_signatures = signatures::lower(
        checked,
        &admitted_bodies,
        &type_ids,
        &domain_ids,
        &structural_types,
        requirements_owner,
        &mut next_place,
        &mut next_value,
    )?;

    let machine_ids = external
        .as_ref()
        .map(|_| &entry)
        .into_iter()
        .chain(closure.iter())
        .chain(&scalar_closure)
        .chain(&selected_structural_scalar_roots)
        .chain(&structural_result_roots)
        .enumerate()
        .map(|(index, symbol)| Ok((*symbol, machine_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut placed_view_inputs = checked
        .facts
        .placed_view_inputs
        .iter()
        .filter(|input| {
            machine_ids
                .iter()
                .any(|(symbol, _)| *symbol == input.machine)
        })
        .map(|input| {
            lower_placed_view_input(
                checked,
                input,
                lookup_machine_id(&machine_ids, input.machine)?,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    placed_view_inputs.sort();
    let scalar_graph_parameters = scalar_callees
        .iter()
        .map(|callee| {
            let parameters = if let CheckedScalarCallee::Graph(_) = callee {
                lower_unit_parameters(
                    callee.structural_parameters(),
                    &type_ids,
                    &domain_ids,
                    &mut next_place,
                )?
            } else {
                Vec::new()
            };
            let locals = if let CheckedScalarCallee::Graph(graph) = callee {
                crate::scalar_graph::scalar_graph_lowering::primitive_locals::allocate(
                    checked,
                    graph,
                    &type_ids,
                    &mut next_place,
                )?
            } else {
                Vec::new()
            };
            Ok((callee.source_machine(), parameters, locals))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let prepared_scalar_machines = scalar_callees
        .into_iter()
        .map(|callee| {
            let source = callee.source_machine();
            let parameters = scalar_graph_parameters
                .iter()
                .find(|(symbol, _, _)| *symbol == source)
                .ok_or(LoweringError::Unsupported(
                    "scalar graph has no allocated parameter namespace",
                ))?;
            callee.prepare(
                checked,
                source,
                &parameters.1,
                &parameters.2,
                &structural_types,
                &mut next_place,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let scalar_requirement_counts = prepared_scalar_machines
        .iter()
        .map(|machine| (machine.source_machine(), machine.requirement_count()))
        .chain(machine_signatures.iter().filter_map(|signature| {
            plans
                .for_machine(signature.source)
                .filter(|plan| plan.scalar_result.is_some() || plan.scalar_control.is_some())
                .map(|_| (signature.source, signature.runtime_requirements.len()))
        }))
        .collect::<Vec<_>>();
    let mut next_operation = 1_u64;
    let mut next_edge = 1_u64;
    let mut next_block = 1_u64;
    let mut next_call_obligation = TERMINAL_UNIT_CALL_OBLIGATION_BASE;
    let mut call_evidence = Vec::new();
    let mut machines = Vec::with_capacity(
        closure.len()
            + scalar_closure.len()
            + selected_structural_scalar_roots.len()
            + structural_result_roots.len(),
    );
    let mut source_call_occurrences = Vec::new();
    let mut selected_ieee_float_fma_occurrences = Vec::new();
    let mut selected_ieee_float_comparison_occurrences = Vec::new();

    for (signature, admitted) in machine_signatures.iter().zip(admitted_bodies) {
        let body = admitted.source();
        let plan = body.entry()?;
        let terminal_machine = lookup_machine_id(&machine_ids, plan.machine)?;
        let parameters = &signature.parameters;
        let scalar_parameters = signature.scalar_parameters.clone();
        let scalar_parameter_count = scalar_parameters.len();
        let runtime_requirements = &signature.runtime_requirements;
        let entry_claims = &signature.claims.entry_claims;
        let claim_bindings = &signature.claims.source_claims;
        if let admission::AdmittedBody::Composed {
            source: source_plan,
            body: admitted,
        } = admitted
        {
            let (machine, mut occurrences) = composed_control::callable::emit(
                checked,
                source_plan,
                admitted,
                parameters.clone(),
                scalar_parameters,
                composed_control::callable::SharedCatalog {
                    structural_types: &structural_types,
                    type_ids: &type_ids,
                    domain_ids: &domain_ids,
                    services: &services,
                    service_ids: &service_ids,
                    root_service_reach: &root_service_reach,
                    boundaries: &boundary_machines,
                    boundary_parameters: &lowered_boundary_parameters,
                    machine_ids: &machine_ids,
                    signatures: &machine_signatures,
                    scalar_requirement_counts: &scalar_requirement_counts,
                },
                composed_control::callable::EmissionCounters {
                    place: &mut next_place,
                    value: &mut next_value,
                    block: &mut next_block,
                    operation: &mut next_operation,
                    edge: &mut next_edge,
                    call_obligation: &mut next_call_obligation,
                },
            )?;
            machines.push(machine);
            source_call_occurrences.append(&mut occurrences);
            continue;
        }
        let plan = body.ordinary()?;
        let content_entry_claims = content_conservation::lower_whole_content_entry_claims(
            checked,
            &structural_types,
            &plan.structural_parameters,
            parameters,
            &plan.entry_claims,
            claim_bindings,
        )?;
        let called_boundaries = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. }
                | CheckedUnitEffectOperationPlan::BoundaryScalarCall { target_machine, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    target_machine, ..
                } => Some(*target_machine),
                _ => None,
            })
            .collect::<Vec<_>>();
        let provider_boundaries = lowered_boundary_parameters
            .iter()
            .map(|(symbol, boundary, _, _)| (*symbol, *boundary))
            .collect::<Vec<_>>();
        let attachment = plan
            .attachment_type_identity
            .as_deref()
            .map(|identity| lookup_type_id(&type_ids, identity))
            .transpose()?;
        let provider_places = if let Some(attachment) = attachment {
            let checked_attachment = plans
                .structural_types
                .iter()
                .find(|declaration| {
                    Some(declaration.identity.as_str()) == plan.attachment_type_identity.as_deref()
                })
                .ok_or(LoweringError::Unsupported(
                    "attached Unit machine is missing its checked attachment shape",
                ))?;
            validate_provider_attachment_requirements(
                checked_attachment,
                &plan.provider_attachment_requirements,
                &called_boundaries,
            )?;
            let attachment_declaration = structural_types
                .iter()
                .find(|declaration| declaration.id == attachment)
                .expect("lowered attachment declaration exists");
            lower_provider_attachment_places(
                attachment,
                attachment_declaration,
                &plan.provider_attachment_requirements,
                &provider_boundaries,
                &mut next_place,
            )?
        } else {
            if !plan.provider_attachment_requirements.is_empty() {
                return unsupported(
                    "free Unit machine fabricates provider-backed attachment requirements",
                );
            }
            Vec::new()
        };
        let local_places = plan
            .trivial_affine_locals
            .iter()
            .map(|local| {
                Ok(StructuralPlaceDeclaration {
                    id: place_id(allocate_dense(&mut next_place)?),
                    kind: StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal: local.declaration_ordinal,
                        structural_type: lookup_type_id(&type_ids, &local.type_identity)?,
                        construction: local
                            .construction
                            .as_ref()
                            .map(|element| {
                                Ok(semantic_vocabulary::AffineConstructionElement {
                                    root_structural_type: lookup_type_id(
                                        &type_ids,
                                        &element.root_type_identity,
                                    )?,
                                    index: element.index,
                                })
                            })
                            .transpose()?,
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
                }
                | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                    structural_arguments,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    structural_arguments,
                    ..
                }
                | CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                }
                | CheckedUnitEffectOperationPlan::StructuralCall {
                    structural_arguments,
                    ..
                } => literal_arguments.extend(
                    structural_arguments
                        .iter()
                        .filter(|argument| argument.byte_sequence_literal().is_some()),
                ),
                _ => {}
            }
        }
        let mut literal_places = literal_arguments
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
            let bytes = argument
                .byte_sequence_literal()
                .ok_or(LoweringError::Unsupported(
                    "byte-sequence literal payload is absent",
                ))?;
            let id = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                id,
                result: terminal_psi::OperationResult::Unit,
                kind: OperationKind::EstablishByteSequenceLiteral {
                    destination: place.id,
                    bytes: bytes.to_vec(),
                },
            });
        }
        let mut next_literal_argument = 0usize;
        let call_literal_count = literal_places.len();
        let mut next_value_identity = next_value;
        let mut scalar_result_values = scalar_parameters.clone();
        let mut primitive_local_places = Vec::<primitive_locals::PrimitiveLocal>::new();
        let mut structural_result_places = Vec::<(StructuralPlaceDeclaration, bool)>::new();
        let mut structural_value_temporaries = Vec::<StructuralPlaceDeclaration>::new();
        let mut evaluation = argument_evaluation::Evaluation::new(&mut next_block)?;
        evaluation.record_fields = crate::scalar_graph::scalar_computations::fields::prepare(
            checked,
            plan.machine,
            &structural_types,
        )?;
        evaluation.arrays = crate::scalar_graph::scalar_computations::arrays::prepare(
            checked,
            plan.machine,
            &structural_types,
            &mut next_place,
        )?;
        evaluation.cases = crate::scalar_graph::scalar_computations::cases::prepare(
            checked,
            plan.machine,
            &structural_types,
            &mut next_place,
        )?;
        evaluation.structural_parameters = plan
            .structural_parameters
            .iter()
            .zip(parameters)
            .map(|(source, parameter)| (source.position, parameter.clone()))
            .collect();
        evaluation.primitive_storage =
            crate::expression_preparation::source_custody::primitive_references::bindings(
                checked,
                plan.state,
                &evaluation.structural_parameters,
                &structural_types,
            )?;
        let mut staged_arguments = vec![Vec::<usize>::new(); plan.operations.len()];
        evaluation.structural_fields =
            crate::expression_preparation::bindings::StructuralScalarFieldBinding::collect(
                &evaluation.structural_parameters,
                &structural_types,
            );
        evaluation.structural_cases =
            crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding::collect(
                &evaluation.structural_parameters,
                &structural_types,
            );
        let mut staged_subslices = vec![Vec::<(usize, PlaceId)>::new(); plan.operations.len()];
        let mut subslice_places = Vec::new();
        let mut retained_scalar_prefix = None;
        let mut staged_scalar_result = None;
        for step in argument_schedule::build(checked, plan)? {
            let mut scalar_calls = CallEmissionContext {
                machine_ids: &machine_ids,
                requirement_counts: &scalar_requirement_counts,
                next_obligation_identity: next_call_obligation,
                obligation_limit: u64::MAX,
            };
            let (operation_index, staged) = match step {
                argument_schedule::Step::Begin => {
                    if retained_scalar_prefix
                        .replace(scalar_result_values.len())
                        .is_some()
                        || staged_scalar_result.is_some()
                    {
                        return unsupported("nested argument staging has an unfinished group");
                    }
                    continue;
                }
                argument_schedule::Step::End => {
                    let prefix =
                        retained_scalar_prefix
                            .take()
                            .ok_or(LoweringError::Unsupported(
                                "nested argument staging has no active group",
                            ))?;
                    scalar_result_values.truncate(prefix);
                    // Argument temporaries are never source-local bindings.
                    // Only the successful outer result extends that namespace.
                    if let Some(result) = staged_scalar_result.take() {
                        scalar_result_values.push(result);
                    }
                    continue;
                }
                argument_schedule::Step::Argument { operation, ordinal } => {
                    let source_value_count = retained_scalar_prefix.ok_or(
                        LoweringError::Unsupported("staged scalar argument has no source prefix"),
                    )?;
                    if staged_arguments[operation].len() != ordinal {
                        return unsupported("staged scalar argument ordinals are not dense");
                    }
                    let value = evaluation.argument_at(
                        checked,
                        plan.machine,
                        plan.state,
                        &plan.operations[operation],
                        ordinal,
                        source_value_count,
                        &mut scalar_result_values,
                        &mut next_value_identity,
                        &mut next_block,
                        &mut next_edge,
                        &mut operations,
                        &mut scalar_calls,
                    )?;
                    next_call_obligation = scalar_calls.next_obligation_identity;
                    staged_arguments[operation].push(scalar_result_values.len());
                    scalar_result_values.push(value);
                    continue;
                }
                argument_schedule::Step::Subslice { operation, ordinal } => {
                    let source_value_count = retained_scalar_prefix.ok_or(
                        LoweringError::Unsupported("subslice staging has no source prefix"),
                    )?;
                    if staged_subslices[operation]
                        .iter()
                        .any(|(prior, _)| *prior == ordinal)
                    {
                        return unsupported("subslice argument is evaluated more than once");
                    }
                    let place = byte_subslices::emit(
                        checked,
                        plan,
                        &plan.operations[operation],
                        ordinal,
                        parameters,
                        &evaluation.structural_parameters,
                        &scalar_result_values[..source_value_count],
                        &type_ids,
                        &mut next_place,
                        &mut next_value_identity,
                        &mut operations,
                    )?;
                    staged_subslices[operation].push((ordinal, place.id));
                    subslice_places.push(place);
                    continue;
                }
                argument_schedule::Step::Call(index) if retained_scalar_prefix.is_some() => {
                    (index, true)
                }
                argument_schedule::Step::Constructor(index) if retained_scalar_prefix.is_some() => {
                    (index, true)
                }
                argument_schedule::Step::Ordinary(index) if retained_scalar_prefix.is_none() => {
                    (index, false)
                }
                _ => return unsupported("call schedule disagrees with its active argument group"),
            };
            let operation = &plan.operations[operation_index];
            if let CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                affine_discards, ..
            } = operation
            {
                let mut discards = Vec::new();
                let mut residuals = Vec::new();
                for discard in affine_discards {
                    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } = discard.source
                    else {
                        return unsupported("call continuation cleanup requires a result binding");
                    };
                    let (place, discard_on_return) = structural_result_places
                        .get(binding_ordinal as usize)
                        .ok_or(LoweringError::Unsupported(
                            "call continuation result has not been produced",
                        ))?;
                    if *discard_on_return {
                        return unsupported("call continuation cleanup has conflicting custody");
                    }
                    if discard.path.is_empty() {
                        discards.push(place.id);
                    } else {
                        residuals.push(terminal_psi::StructuralAffineDiscard {
                            place: place.id,
                            path: lower_structural_path(&discard.path),
                            structural_type: lookup_type_id(&type_ids, &discard.type_identity)?,
                        });
                    }
                }
                evaluation.cleanup_continuation(
                    discards,
                    residuals,
                    &mut scalar_result_values,
                    &mut next_value_identity,
                    &mut next_block,
                    &mut next_edge,
                    &operations,
                )?;
                continue;
            }
            let source_value_count = retained_scalar_prefix.unwrap_or(scalar_result_values.len());
            let evaluated_scalar_arguments = if staged {
                Some(
                    staged_arguments[operation_index]
                        .iter()
                        .map(|index| {
                            scalar_result_values.get(*index).copied().ok_or(
                                LoweringError::Unsupported(
                                    "staged scalar operand lost its retained value",
                                ),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
            } else {
                evaluation.arguments(
                    checked,
                    plan.machine,
                    plan.state,
                    operation,
                    &mut scalar_result_values,
                    &mut next_value_identity,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                    &mut scalar_calls,
                )?
            };
            next_call_obligation = scalar_calls.next_obligation_identity;
            let mut source_call = None;
            let kind = match operation {
                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    discard_result_on_return,
                    ..
                } => {
                    let mut emit_operand_call =
                        |operand: &CheckedUnitEffectOperationPlan,
                         evaluated: Option<&[ValueDeclaration]>,
                         call_context: &mut CallEmissionContext<'_>,
                         output: &mut OperationBuffer,
                         place_counter: &mut u64| {
                            let CheckedUnitEffectOperationPlan::StructuralCall {
                                target_machine,
                                result,
                                ..
                            } = operand
                            else {
                                return unsupported("record operand is not a structural call");
                            };
                            if result.binding_ordinal as usize != structural_result_places.len() {
                                return unsupported(
                                    "structural operand result binding is not dense",
                                );
                            }
                            let prepared = ordinary_calls::prepare(
                                checked,
                                plans,
                                operand,
                                signatures::find(&machine_signatures, *target_machine)?
                                    .call_target(),
                                evaluated,
                                parameters,
                                &local_places,
                                &structural_result_places,
                                &primitive_local_places,
                                &type_ids,
                                &structural_types,
                                &[],
                                call_context,
                            )?;
                            let declaration = ordinary_calls::emit_structural(
                                checked,
                                plan.state,
                                operand,
                                prepared,
                                lookup_machine_id(&machine_ids, *target_machine)?,
                                &type_ids,
                                &domain_ids,
                                claim_bindings,
                                true,
                                place_counter,
                                output,
                            )?;
                            structural_result_places.push((declaration, false));
                            Ok(declaration)
                        };
                    let declaration = structural_values::emit(
                        checked,
                        plan.machine,
                        plan.state,
                        operation,
                        &structural_types,
                        &type_ids,
                        &mut next_place,
                        &mut structural_value_temporaries,
                        &mut emit_operand_call,
                        &mut scalar_calls,
                        &mut evaluation,
                        &mut scalar_result_values,
                        &mut next_value_identity,
                        &mut next_block,
                        &mut next_edge,
                        &mut operations,
                    )?;
                    next_call_obligation = scalar_calls.next_obligation_identity;
                    if result.binding_ordinal as usize != structural_result_places.len() {
                        return unsupported("structural value result binding is not dense");
                    }
                    structural_result_places.push((declaration, *discard_result_on_return));
                    continue;
                }
                CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                    statement_index,
                    symbol,
                    type_identity,
                    primitive_type,
                    ..
                } => {
                    if primitive_local_places
                        .iter()
                        .any(|local| local.symbol == *symbol)
                    {
                        return unsupported("primitive local is established more than once");
                    }
                    let value = crate::expression_preparation::bindings::ScalarBindings::new(
                        source_value_count,
                    )
                    .with_primitive_storage(&evaluation.primitive_storage)
                    .expression_at(
                        checked,
                        plan.state,
                        *statement_index,
                        CheckedScalarExpressionRole::StorageInitializer,
                    )?;
                    let structural_type = lookup_type_id(&type_ids, type_identity)?;
                    if value.scalar_type() != terminal_scalar_type(*primitive_type)?
                        || !structural_types.iter().any(|declaration| {
                            declaration.id == structural_type
                                && declaration.shape
                                    == StructuralTypeShape::PrimitiveScalar(value.scalar_type())
                        })
                    {
                        return unsupported(
                            "primitive local initializer and referent types disagree",
                        );
                    }
                    let local = primitive_locals::emit(
                        *symbol,
                        structural_type,
                        &value,
                        &scalar_result_values,
                        &mut next_place,
                        &mut next_value_identity,
                        &mut operations,
                    )?;
                    evaluation.primitive_storage.push((
                        local.symbol,
                        local.declaration.id,
                        local.scalar_type,
                    ));
                    primitive_local_places.push(local);
                    continue;
                }
                CheckedUnitEffectOperationPlan::EstablishReference { result, source } => {
                    if result.binding_ordinal as usize != structural_result_places.len() {
                        return unsupported("reference result binding is not dense");
                    }
                    let arguments = parameters::lower_structural_arguments(
                        std::slice::from_ref(source),
                        parameters,
                        &local_places,
                        &structural_result_places,
                        &[],
                        &primitive_local_places,
                    )?;
                    let declaration = reference_results::emit(
                        result,
                        arguments[0].clone(),
                        &type_ids,
                        &mut next_place,
                        &mut operations,
                    )?;
                    structural_result_places.push((declaration, false));
                    continue;
                }
                CheckedUnitEffectOperationPlan::ReleaseReference {
                    binding_ordinal, ..
                } => {
                    let source = structural_result_places
                        .get(*binding_ordinal as usize)
                        .ok_or(LoweringError::Unsupported(
                            "released reference result is absent",
                        ))?
                        .0
                        .id;
                    OperationKind::ReleaseReference {
                        source: evaluation.current_structural_place(source),
                    }
                }
                CheckedUnitEffectOperationPlan::EstablishScalarArray {
                    source,
                    result,
                    elements,
                } => {
                    if result.binding_ordinal as usize != structural_result_places.len() {
                        return unsupported("array result binding is not dense");
                    }
                    // Earlier arguments are private staging slots, not source
                    // bindings. Keep them live through every leaf's control
                    // joins, then remove only this constructor's leaf tail.
                    let leaf_start = scalar_result_values.len();
                    let source_value_count = retained_scalar_prefix.unwrap_or(leaf_start);
                    for (ordinal, element) in elements.iter().enumerate() {
                        let element_ordinal = u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("array element ordinal exceeds u32")
                        })?;
                        let value = evaluation.source_value(
                            checked,
                            plan.machine,
                            plan.state,
                            result.statement_index,
                            CheckedScalarExpressionRole::ArrayElement {
                                source: *source,
                                element_ordinal,
                            },
                            element,
                            source_value_count,
                            &mut scalar_result_values,
                            &mut next_value_identity,
                            &mut next_block,
                            &mut next_edge,
                            &mut operations,
                            &mut scalar_calls,
                        )?;
                        scalar_result_values.push(value);
                    }
                    next_call_obligation = scalar_calls.next_obligation_identity;
                    // Private control joins may replace every scalar identity.
                    // Read the completed leaves only after the final element.
                    let declaration = scalar_arrays::emit(
                        result,
                        &scalar_result_values[leaf_start..],
                        &type_ids,
                        &mut next_place,
                        &mut operations,
                    )?;
                    scalar_result_values.truncate(leaf_start);
                    structural_result_places.push((declaration, false));
                    if *source == checked_trees::CheckedArrayConstructionSource::Statement {
                        structural_values::bind_local(
                            checked,
                            plan,
                            operation,
                            declaration.id,
                            &mut evaluation.structural_locals,
                        )?;
                    }
                    continue;
                }
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
                            ..
                        } if ordinal == *declaration_ordinal
                            && structural_type == lookup_type_id(&type_ids, type_identity)?
                    ) {
                        return unsupported("Unit local declaration drifted from checked custody");
                    }
                    OperationKind::EstablishTrivialAffineLocal {
                        destination: local.id,
                    }
                }
                CheckedUnitEffectOperationPlan::StructuralCall { target_machine, .. }
                    if !UnitBody::contains(plans, *target_machine) =>
                {
                    let result = structural_calls::emit(
                        checked,
                        plan,
                        operation,
                        parameters,
                        evaluated_scalar_arguments.as_deref(),
                        &structural_types,
                        &type_ids,
                        &machine_ids,
                        &structural_result_places,
                        &mut next_place,
                        &mut operations,
                    )?;
                    structural_values::bind_local(
                        checked,
                        plan,
                        operation,
                        result.0.id,
                        &mut evaluation.structural_locals,
                    )?;
                    structural_result_places.push(result);
                    continue;
                }
                CheckedUnitEffectOperationPlan::CallUnit {
                    coordinate,
                    target_machine,
                    target_state,
                    structural_arguments,
                    ..
                }
                | CheckedUnitEffectOperationPlan::StructuralCall {
                    coordinate,
                    target_machine,
                    target_state,
                    structural_arguments,
                    ..
                } => {
                    let claim_transfers = match operation {
                        CheckedUnitEffectOperationPlan::CallUnit {
                            claim_transfers, ..
                        } => claim_transfers.as_slice(),
                        _ => &[],
                    };
                    let call_byte_places = byte_subslices::argument_places(
                        structural_arguments,
                        &literal_places,
                        &mut next_literal_argument,
                        &staged_subslices[operation_index],
                    )?;
                    scalar_calls.next_obligation_identity = next_call_obligation;
                    let ordinary_calls::PreparedCall {
                        arguments: terminal_scalar_arguments,
                        structural_arguments: terminal_arguments,
                        requirement_obligations,
                        crash_continuations,
                    } = ordinary_calls::prepare(
                        checked,
                        plans,
                        operation,
                        signatures::find(&machine_signatures, *target_machine)?.call_target(),
                        evaluated_scalar_arguments.as_deref(),
                        parameters,
                        &local_places,
                        &structural_result_places,
                        &primitive_local_places,
                        &type_ids,
                        &structural_types,
                        &call_byte_places,
                        &mut scalar_calls,
                    )?;
                    next_call_obligation = scalar_calls.next_obligation_identity;
                    source_call = Some((*coordinate, None, *target_state));
                    if let CheckedUnitEffectOperationPlan::StructuralCall {
                        discard_result_on_return,
                        ..
                    } = operation
                    {
                        let declaration = ordinary_calls::emit_structural(
                            checked,
                            plan.state,
                            operation,
                            ordinary_calls::PreparedCall {
                                arguments: terminal_scalar_arguments,
                                structural_arguments: terminal_arguments,
                                requirement_obligations,
                                crash_continuations,
                            },
                            lookup_machine_id(&machine_ids, *target_machine)?,
                            &type_ids,
                            &domain_ids,
                            claim_bindings,
                            true,
                            &mut next_place,
                            &mut operations,
                        )?;
                        let place = declaration.id;
                        structural_result_places.push((declaration, *discard_result_on_return));
                        structural_values::bind_local(
                            checked,
                            plan,
                            operation,
                            place,
                            &mut evaluation.structural_locals,
                        )?;
                        continue;
                    }
                    OperationKind::CallUnit {
                        callee: lookup_machine_id(&machine_ids, *target_machine)?,
                        arguments: terminal_scalar_arguments,
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
                CheckedUnitEffectOperationPlan::ScalarCall {
                    coordinate,
                    result,
                    target_machine: realization_machine,
                    target_state: realization_state,
                    ..
                }
                | CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                    coordinate,
                    result,
                    realization_machine,
                    realization_state,
                    ..
                } => {
                    let source_target = match operation {
                        CheckedUnitEffectOperationPlan::ScalarCall { target_state, .. } => {
                            *target_state
                        }
                        CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                            realization_machine,
                            ..
                        } => *realization_machine,
                        _ => unreachable!("combined Unit scalar call arm"),
                    };
                    if usize::try_from(result.binding_ordinal)
                        .ok()
                        .and_then(|ordinal| ordinal.checked_add(scalar_parameter_count))
                        != Some(source_value_count)
                    {
                        return unsupported(
                            "Unit scalar result binding ordinal drifted from source order",
                        );
                    }
                    let target = match operation {
                        CheckedUnitEffectOperationPlan::ScalarCall { .. } => {
                            CheckedScalarCallee::find_for_unit_call(checked, *realization_machine)?
                        }
                        _ => CheckedScalarCallee::find(checked, *realization_machine)?,
                    };
                    // Both catalogs have been source-validated before emission.
                    // An operation-body callee uses its real ordered emitter,
                    // not a fabricated PreparedScalarCallee graph.
                    let target_result = match prepared_scalar_machines
                        .iter()
                        .find(|prepared| prepared.source_machine() == *realization_machine)
                    {
                        Some(prepared) => prepared.result_type(),
                        None if closure.contains(realization_machine)
                            && matches!(target, CheckedScalarCallee::Operations(_)) =>
                        {
                            target.result_type()?
                        }
                        None => {
                            return unsupported(
                                "Unit scalar call target is absent from the prepared closure",
                            );
                        }
                    };
                    let target_parameter_types = target.parameter_types()?;
                    if target.entry_state()? != *realization_state
                        || target_result != terminal_scalar_type(result.primitive_type)?
                    {
                        return unsupported(
                            "Unit scalar call disagrees with its prepared target signature",
                        );
                    }
                    let arguments = if let Some(arguments) = evaluated_scalar_arguments.as_deref() {
                        argument_evaluation::validated_values(
                            Some(arguments),
                            &target_parameter_types
                                .iter()
                                .map(|primitive| terminal_scalar_type(*primitive))
                                .collect::<Result<Vec<_>, _>>()?,
                        )?
                    } else {
                        let CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                            scalar_arguments,
                            ..
                        } = operation
                        else {
                            return unsupported("Unit scalar call has no evaluated arguments");
                        };
                        let source_types = scalar_result_values
                            .iter()
                            .map(|value| value.scalar_type)
                            .collect::<Vec<_>>();
                        if scalar_arguments.len() != target_parameter_types.len() {
                            return unsupported("selected scalar call argument count disagrees");
                        }
                        scalar_arguments.iter().zip(&target_parameter_types).map(|(argument, primitive)| {
                            let argument = lower_checked_scalar_expression(argument)?;
                            let scalar_type = terminal_scalar_type(*primitive)?;
                            if argument.scalar_type() != scalar_type || direct_expression_contains_short_circuit(&argument) {
                                return unsupported("selected scalar call has unsupported argument control or carrier");
                            }
                            validate_direct_parameter_types(&argument, &source_types)?;
                            Ok(ValueDeclaration {
qualifications: Default::default(), id: emit_direct_expression(&argument, &scalar_result_values, &mut next_value_identity, &mut operations), scalar_type })
                        }).collect::<Result<Vec<_>, LoweringError>>()?
                    };
                    let target_contract = checked
                        .facts
                        .contract_plans
                        .for_machine(*realization_machine)
                        .ok_or(LoweringError::Unsupported(
                            "Unit scalar call target has no checked contract",
                        ))?;
                    let crash_continuations = lower_checked_crash_route_buckets(
                        target_contract.crash.published(),
                        &arguments,
                    )?;
                    let requirement_count = scalar_requirement_counts
                        .iter()
                        .find_map(|(source, count)| {
                            (*source == *realization_machine).then_some(*count)
                        })
                        .ok_or(LoweringError::Unsupported(
                            "Unit scalar call target has no prepared contract",
                        ))?;
                    let requirement_obligations = (0..requirement_count)
                        .map(|_| {
                            let obligation = obligation_id(next_call_obligation);
                            next_call_obligation = next_call_obligation.checked_add(1).ok_or(
                                LoweringError::Unsupported(
                                    "Unit scalar call obligation identity space is exhausted",
                                ),
                            )?;
                            Ok(obligation)
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    let value = ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(next_value_identity),
                        scalar_type: target_result,
                    };
                    next_value_identity =
                        next_value_identity
                            .checked_add(1)
                            .ok_or(LoweringError::Unsupported(
                                "Unit scalar result value identity space is exhausted",
                            ))?;
                    let operation_id = operations.allocate();
                    operations.record_source_call(
                        SourceCallCoordinate {
                            state: plan.state,
                            statement_index: usize::try_from(coordinate.statement_index).map_err(
                                |_| {
                                    LoweringError::Unsupported(
                                        "Unit scalar call statement coordinate exceeds usize",
                                    )
                                },
                            )?,
                            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(
                                |_| {
                                    LoweringError::Unsupported(
                                        "Unit scalar call ordinal coordinate exceeds usize",
                                    )
                                },
                            )?,
                        },
                        None,
                        operation_id,
                        source_target,
                    )?;
                    let callee = lookup_machine_id(&machine_ids, *realization_machine)?;
                    let arguments = arguments.iter().map(|argument| argument.id).collect();
                    let kind = if let CheckedUnitEffectOperationPlan::ScalarCall {
                        structural_arguments,
                        claim_transfers,
                        ..
                    } = operation
                        && (!structural_arguments.is_empty()
                            || !claim_transfers.is_empty()
                            || target.requires_structural_frame())
                    {
                        if target.structural_parameters().is_empty()
                            && !structural_arguments.is_empty()
                        {
                            return unsupported(
                                "structural scalar call has no structural checked body",
                            );
                        }
                        validate_transfer_shape(
                            structural_arguments,
                            claim_transfers,
                            parameters,
                            &local_places,
                            &structural_result_places,
                            target.structural_parameters(),
                            &type_ids,
                            &structural_types,
                            &target
                                .entry_claims()
                                .iter()
                                .map(|claim| claim.parameter_index)
                                .collect::<Vec<_>>(),
                            &primitive_local_places,
                            None,
                        )?;
                        OperationKind::CallStructuralScalar {
                            callee,
                            arguments,
                            structural_arguments: lower_structural_arguments(
                                structural_arguments,
                                parameters,
                                &local_places,
                                &structural_result_places,
                                &[],
                                &primitive_local_places,
                            )?,
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
                    } else {
                        OperationKind::Call {
                            callee,
                            arguments,
                            requirement_obligations,
                            crash_continuations,
                        }
                    };
                    operations.push(Operation {
                        static_reach_binding: None,
                        id: operation_id,
                        result: terminal_psi::OperationResult::Scalar(value),
                        kind,
                    });
                    if staged {
                        if staged_scalar_result.replace(value).is_some() {
                            return unsupported(
                                "nested argument group produces more than one scalar binding",
                            );
                        }
                    } else if !crate::emission::call_source_custody::initializers::discards_result(
                        checked,
                        plan.state,
                        *coordinate,
                    )? {
                        scalar_result_values.push(value);
                    }
                    continue;
                }
                CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value } => {
                    if usize::try_from(result.binding_ordinal)
                        .ok()
                        .and_then(|ordinal| ordinal.checked_add(scalar_parameter_count))
                        != Some(scalar_result_values.len())
                    {
                        return unsupported(
                            "Unit scalar expression local binding drifted from source order",
                        );
                    }
                    let role = if plan.scalar_result.as_ref() == Some(result) {
                        CheckedScalarExpressionRole::Return
                    } else {
                        CheckedScalarExpressionRole::LocalInitializer {
                            binding_ordinal: result.binding_ordinal,
                        }
                    };
                    let lowered = evaluation.source_value(
                        checked,
                        plan.machine,
                        plan.state,
                        result.statement_index,
                        role,
                        value,
                        source_value_count,
                        &mut scalar_result_values,
                        &mut next_value_identity,
                        &mut next_block,
                        &mut next_edge,
                        &mut operations,
                        &mut scalar_calls,
                    )?;
                    if lowered.scalar_type != terminal_scalar_type(result.primitive_type)? {
                        return unsupported(
                            "Unit scalar expression local type disagrees with its binding",
                        );
                    }
                    next_call_obligation = scalar_calls.next_obligation_identity;
                    scalar_result_values.push(lowered);
                    continue;
                }
                CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                    coordinate,
                    result,
                    realization_machine,
                    realization_state,
                    scalar_arguments,
                    structural_arguments,
                    ..
                } => {
                    if usize::try_from(result.binding_ordinal)
                        .ok()
                        .and_then(|ordinal| ordinal.checked_add(scalar_parameter_count))
                        != Some(scalar_result_values.len())
                    {
                        return unsupported(
                            "selected structural Unit operator result binding ordinal drifted from source order",
                        );
                    }
                    let realizations = checked
                        .facts
                        .flow
                        .terminal_structural_scalar_returns
                        .machines
                        .iter()
                        .filter(|target| {
                            target.machine == *realization_machine
                                && target.state == *realization_state
                        })
                        .collect::<Vec<_>>();
                    let [target] = realizations.as_slice() else {
                        return unsupported(
                            "selected structural Unit operator target is absent or ambiguous",
                        );
                    };
                    validate_transfer_shape(
                        structural_arguments,
                        &[],
                        parameters,
                        &[],
                        &[],
                        &target.structural_parameters,
                        &type_ids,
                        &structural_types,
                        &[],
                        &primitive_local_places,
                        None,
                    )?;
                    if scalar_arguments.len() != target.scalar_parameters.len() {
                        return unsupported(
                            "selected structural Unit operator scalar argument count disagrees with its realization",
                        );
                    }
                    let source_types = scalar_result_values
                        .iter()
                        .map(|value| value.scalar_type)
                        .collect::<Vec<_>>();
                    let scalar_arguments = scalar_arguments
                        .iter()
                        .zip(&target.scalar_parameters)
                        .map(|(argument, target)| {
                            let argument = lower_checked_scalar_expression(argument)?;
                            if direct_expression_contains_short_circuit(&argument) {
                                return unsupported(
                                    "selected structural Unit operator arguments do not yet admit short-circuit control",
                                );
                            }
                            let target_type = terminal_scalar_type(target.primitive_type)?;
                            if argument.scalar_type() != target_type {
                                return unsupported(
                                    "selected structural Unit operator scalar argument type disagrees with its realization",
                                );
                            }
                            validate_direct_parameter_types(&argument, &source_types)?;
                            Ok(emit_direct_expression(
                                &argument,
                                &scalar_result_values,
                                &mut next_value_identity,
                                &mut operations,
                            ))
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    let arguments = lower_structural_arguments(
                        structural_arguments,
                        parameters,
                        &[],
                        &[],
                        &[],
                        &primitive_local_places,
                    )?;
                    let value = ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(next_value_identity),
                        scalar_type: terminal_scalar_type(result.primitive_type)?,
                    };
                    next_value_identity =
                        next_value_identity
                            .checked_add(1)
                            .ok_or(LoweringError::Unsupported(
                                "selected structural scalar result identity space is exhausted",
                            ))?;
                    let operation_id = operations.allocate();
                    operations.record_source_call(
                        SourceCallCoordinate {
                            state: plan.state,
                            statement_index: usize::try_from(coordinate.statement_index).map_err(
                                |_| {
                                    LoweringError::Unsupported(
                                        "selected structural scalar statement coordinate exceeds usize",
                                    )
                                },
                            )?,
                            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                                LoweringError::Unsupported(
                                    "selected structural scalar call ordinal exceeds usize",
                                )
                            })?,
                        },
                        None,
                        operation_id,
                        *realization_machine,
                    )?;
                    operations.push(Operation {
                        static_reach_binding: None,
                        id: operation_id,
                        result: OperationResult::Scalar(value),
                        kind: OperationKind::CallStructuralScalar {
                            callee: lookup_machine_id(&machine_ids, *realization_machine)?,
                            arguments: scalar_arguments,
                            structural_arguments: arguments,
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                    });
                    scalar_result_values.push(value);
                    continue;
                }
                CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                    coordinate,
                    result,
                    realization_machine,
                    realization_state,
                    scalar_arguments,
                    structural_arguments,
                    discard_result_on_return,
                    ..
                } => {
                    let realizations = checked
                        .facts
                        .flow
                        .terminal_structural_returns
                        .claim_free_affine_machines
                        .iter()
                        .filter(|target| {
                            target.machine == *realization_machine
                                && target.state == *realization_state
                        })
                        .collect::<Vec<_>>();
                    let [target] = realizations.as_slice() else {
                        return unsupported(
                            "selected structural-result Unit operator target is absent or ambiguous",
                        );
                    };
                    validate_transfer_shape(
                        structural_arguments,
                        &[],
                        parameters,
                        &[],
                        &[],
                        std::slice::from_ref(&target.structural_parameter),
                        &type_ids,
                        &structural_types,
                        &[],
                        &primitive_local_places,
                        None,
                    )?;
                    if scalar_arguments.len() != target.scalar_parameters.len() {
                        return unsupported(
                            "selected structural-result scalar argument count disagrees with its realization",
                        );
                    }
                    let source_types = scalar_result_values
                        .iter()
                        .map(|value| value.scalar_type)
                        .collect::<Vec<_>>();
                    let scalar_arguments = scalar_arguments
                        .iter()
                        .zip(&target.scalar_parameters)
                        .map(|(argument, target)| {
                            let argument = lower_checked_scalar_expression(argument)?;
                            if direct_expression_contains_short_circuit(&argument) {
                                return unsupported(
                                    "selected structural-result arguments do not admit short-circuit control",
                                );
                            }
                            let target_type = terminal_scalar_type(target.primitive_type)?;
                            if argument.scalar_type() != target_type {
                                return unsupported(
                                    "selected structural-result scalar argument type disagrees with its realization",
                                );
                            }
                            validate_direct_parameter_types(&argument, &source_types)?;
                            Ok(emit_direct_expression(
                                &argument,
                                &scalar_result_values,
                                &mut next_value_identity,
                                &mut operations,
                            ))
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    let arguments = lower_structural_arguments(
                        structural_arguments,
                        parameters,
                        &[],
                        &[],
                        &[],
                        &primitive_local_places,
                    )?;
                    let operation_id = operations.allocate();
                    let result_place = place_id(allocate_dense(&mut next_place)?);
                    let result_type = lookup_type_id(&type_ids, &result.type_identity)?;
                    let result_declaration = StructuralPlaceDeclaration {
                        id: result_place,
                        kind: StructuralPlaceKind::OperationResult {
                            producer: operation_id,
                            structural_type: result_type,
                        },
                    };
                    operations.record_source_call(
                        SourceCallCoordinate {
                            state: plan.state,
                            statement_index: usize::try_from(coordinate.statement_index).map_err(
                                |_| {
                                    LoweringError::Unsupported(
                                        "selected structural-result statement coordinate exceeds usize",
                                    )
                                },
                            )?,
                            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                                LoweringError::Unsupported(
                                    "selected structural-result call ordinal exceeds usize",
                                )
                            })?,
                        },
                        None,
                        operation_id,
                        *realization_machine,
                    )?;
                    operations.push(Operation {
                        static_reach_binding: None,
                        id: operation_id,
                        result: OperationResult::Structural(StructuralOperationResult {
                            place: result_place,
                            structural_type: result_type,
                            multiplicity: StructuralMultiplicity::Affine,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            claims: Vec::new(),
                        }),
                        kind: OperationKind::CallStructuralWithScalarArguments {
                            callee: lookup_machine_id(&machine_ids, *realization_machine)?,
                            arguments: scalar_arguments,
                            structural_arguments: arguments,
                            claim_transfers: Vec::new(),
                            returned_claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                    });
                    structural_result_places.push((result_declaration, *discard_result_on_return));
                    continue;
                }
                CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
                    coordinate,
                    result,
                    requirement_operator,
                    provider_plan_report_fingerprint,
                    provider_plan_commitment,
                    format,
                    operands,
                } => {
                    if usize::try_from(result.binding_ordinal)
                        .ok()
                        .and_then(|ordinal| ordinal.checked_add(scalar_parameter_count))
                        != Some(scalar_result_values.len())
                    {
                        return unsupported(
                            "selected IEEE FMA result binding ordinal drifted from source order",
                        );
                    }
                    let result_type = terminal_scalar_type(result.primitive_type)?;
                    if result_type != ScalarType::IeeeFloat(*format) {
                        return unsupported(
                            "selected IEEE FMA result type disagrees with its exact format",
                        );
                    }
                    let [left, right, addend] = operands.as_slice() else {
                        return unsupported("selected IEEE FMA must retain exactly three operands");
                    };
                    let source_types = scalar_result_values
                        .iter()
                        .map(|value| value.scalar_type)
                        .collect::<Vec<_>>();
                    let mut lower_operand = |operand: &CheckedScalarExpression| {
                        let operand = lower_checked_scalar_expression(operand)?;
                        if direct_expression_contains_short_circuit(&operand) {
                            return unsupported(
                                "selected IEEE FMA operands do not admit short-circuit control",
                            );
                        }
                        if operand.scalar_type() != result_type {
                            return unsupported(
                                "selected IEEE FMA operand type disagrees with its result",
                            );
                        }
                        validate_direct_parameter_types(&operand, &source_types)?;
                        Ok(emit_direct_expression(
                            &operand,
                            &scalar_result_values,
                            &mut next_value_identity,
                            &mut operations,
                        ))
                    };
                    let left = lower_operand(left)?;
                    let right = lower_operand(right)?;
                    let addend = lower_operand(addend)?;
                    let value = ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(next_value_identity),
                        scalar_type: result_type,
                    };
                    next_value_identity =
                        next_value_identity
                            .checked_add(1)
                            .ok_or(LoweringError::Unsupported(
                                "selected IEEE FMA result value identity space is exhausted",
                            ))?;
                    let operation = operations.allocate();
                    operations.record_selected_ieee_float_fma(
                        SourceCallCoordinate {
                            state: plan.state,
                            statement_index: usize::try_from(coordinate.statement_index).map_err(
                                |_| {
                                    LoweringError::Unsupported(
                                        "selected IEEE FMA statement coordinate exceeds usize",
                                    )
                                },
                            )?,
                            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(
                                |_| {
                                    LoweringError::Unsupported(
                                        "selected IEEE FMA call ordinal exceeds usize",
                                    )
                                },
                            )?,
                        },
                        operation,
                        *requirement_operator,
                        *provider_plan_report_fingerprint,
                        *provider_plan_commitment,
                        *format,
                    )?;
                    operations.push(Operation {
                        static_reach_binding: None,
                        id: operation,
                        result: terminal_psi::OperationResult::Scalar(value),
                        kind: OperationKind::NearestIeeeFloatFusedMultiplyAdd {
                            left,
                            right,
                            addend,
                        },
                    });
                    scalar_result_values.push(value);
                    continue;
                }
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    coordinate,
                    source_site,
                    target_machine,
                    scalar_arguments,
                    structural_arguments,
                    completion_receipts,
                    ..
                } => {
                    source_call = Some((*coordinate, *source_site, *target_machine));
                    let target = unique_unit_boundary(plans, *target_machine)?;
                    structural_calls::validate_consumer(
                        checked,
                        plan,
                        operation,
                        &target.structural_parameters,
                        &[],
                    )?;
                    let expected_claim_arguments = structural_arguments
                        .iter()
                        .enumerate()
                        .flat_map(|(argument_index, argument)| {
                            plan.entry_claims
                                .iter()
                                .filter(move |claim| {
                                    argument.byte_sequence_literal().is_none()
                                        && Some(claim.parameter_index)
                                            == argument.source_parameter_index()
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
                        &[],
                        &structural_result_places,
                        &target.structural_parameters,
                        &type_ids,
                        &structural_types,
                        &expected_claim_arguments,
                        &primitive_local_places,
                        None,
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
                    let arguments = argument_evaluation::validated_values(
                        evaluated_scalar_arguments.as_deref(),
                        target_scalar_parameters,
                    )?
                    .iter()
                    .map(|value| value.id)
                    .collect();
                    let call_byte_places = byte_subslices::argument_places(
                        structural_arguments,
                        &literal_places,
                        &mut next_literal_argument,
                        &staged_subslices[operation_index],
                    )?;
                    OperationKind::BoundaryCall {
                        boundary: *boundary,
                        arguments,
                        structural_arguments: lower_structural_arguments(
                            structural_arguments,
                            parameters,
                            &[],
                            &structural_result_places,
                            &call_byte_places,
                            &primitive_local_places,
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
                    }
                }
                CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                    coordinate,
                    source_site,
                    result,
                    target_machine,
                    scalar_arguments,
                    structural_arguments,
                    completion_receipts,
                    ..
                } => {
                    if usize::try_from(result.binding_ordinal)
                        .ok()
                        .and_then(|ordinal| ordinal.checked_add(scalar_parameter_count))
                        != Some(source_value_count)
                    {
                        return unsupported(
                            "Unit scalar result binding ordinal drifted from source order",
                        );
                    }
                    let target = unique_unit_boundary(plans, *target_machine)?;
                    if target.result.scalar() != Some(result.primitive_type) {
                        return unsupported(
                            "Unit scalar result type drifted from its checked boundary target",
                        );
                    }
                    structural_calls::validate_consumer(
                        checked,
                        plan,
                        operation,
                        &target.structural_parameters,
                        &[],
                    )?;
                    let expected_claim_arguments = structural_arguments
                        .iter()
                        .enumerate()
                        .flat_map(|(argument_index, argument)| {
                            plan.entry_claims
                                .iter()
                                .filter(move |claim| {
                                    argument.byte_sequence_literal().is_none()
                                        && Some(claim.parameter_index)
                                            == argument.source_parameter_index()
                                        && (argument.path.is_empty() || claim.path == argument.path)
                                })
                                .map(move |_| {
                                    u32::try_from(argument_index).map_err(|_| {
                                        LoweringError::Unsupported(
                                            "boundary scalar argument index exceeds u32",
                                        )
                                    })
                                })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    validate_transfer_shape(
                        structural_arguments,
                        completion_receipts,
                        parameters,
                        &[],
                        &structural_result_places,
                        &target.structural_parameters,
                        &type_ids,
                        &structural_types,
                        &expected_claim_arguments,
                        &primitive_local_places,
                        None,
                    )?;
                    let (_, boundary, _, target_scalar_parameters) = lowered_boundary_parameters
                        .iter()
                        .find(|(symbol, _, _, _)| *symbol == *target_machine)
                        .ok_or(LoweringError::Unsupported(
                            "boundary scalar call target is absent from the lowered closure",
                        ))?;
                    if scalar_arguments.len() != target_scalar_parameters.len() {
                        return unsupported(
                            "boundary scalar argument count disagrees with its declaration",
                        );
                    }
                    let arguments = argument_evaluation::validated_values(
                        evaluated_scalar_arguments.as_deref(),
                        target_scalar_parameters,
                    )?
                    .iter()
                    .map(|value| value.id)
                    .collect();
                    let call_byte_places = byte_subslices::argument_places(
                        structural_arguments,
                        &literal_places,
                        &mut next_literal_argument,
                        &staged_subslices[operation_index],
                    )?;
                    let kind = OperationKind::BoundaryCall {
                        boundary: *boundary,
                        arguments,
                        structural_arguments: lower_structural_arguments(
                            structural_arguments,
                            parameters,
                            &[],
                            &structural_result_places,
                            &call_byte_places,
                            &primitive_local_places,
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
                    };
                    let value = ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(next_value_identity),
                        scalar_type: terminal_scalar_type(result.primitive_type)?,
                    };
                    next_value_identity =
                        next_value_identity
                            .checked_add(1)
                            .ok_or(LoweringError::Unsupported(
                                "Unit scalar result value identity space is exhausted",
                            ))?;
                    let id = operations.allocate();
                    operations.record_source_call(
                        SourceCallCoordinate {
                            state: plan.state,
                            statement_index: usize::try_from(coordinate.statement_index).map_err(
                                |_| {
                                    LoweringError::Unsupported(
                                        "boundary scalar call statement coordinate exceeds usize",
                                    )
                                },
                            )?,
                            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(
                                |_| {
                                    LoweringError::Unsupported(
                                        "boundary scalar call ordinal coordinate exceeds usize",
                                    )
                                },
                            )?,
                        },
                        *source_site,
                        id,
                        *target_machine,
                    )?;
                    operations.push(Operation {
                        static_reach_binding: None,
                        id,
                        result: terminal_psi::OperationResult::Scalar(value),
                        kind,
                    });
                    if staged {
                        if staged_scalar_result.replace(value).is_some() {
                            return unsupported(
                                "nested argument group produces more than one scalar binding",
                            );
                        }
                    } else if !crate::emission::call_source_custody::initializers::discards_result(
                        checked,
                        plan.state,
                        *coordinate,
                    )? {
                        scalar_result_values.push(value);
                    }
                    continue;
                }
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    coordinate,
                    source_site,
                    result,
                    target_machine,
                    scalar_arguments,
                    structural_arguments,
                    completion_receipts,
                    discard_result_on_return,
                    ..
                } => {
                    if usize::try_from(result.binding_ordinal).ok()
                        != Some(structural_result_places.len())
                    {
                        return unsupported(
                            "Unit structural result binding ordinal drifted from source order",
                        );
                    }
                    let target = unique_unit_boundary(plans, *target_machine)?;
                    let CheckedBoundaryMachineResultPlan::Structural {
                        type_identity,
                        multiplicity,
                        qualifications,
                    } = &target.result
                    else {
                        return unsupported(
                            "Unit structural result target is not a structural boundary",
                        );
                    };
                    if type_identity != &result.type_identity
                        || multiplicity != &result.multiplicity
                    {
                        return unsupported(
                            "Unit structural result drifted from its checked boundary target",
                        );
                    }
                    structural_calls::validate_consumer(
                        checked,
                        plan,
                        operation,
                        &target.structural_parameters,
                        &[],
                    )?;
                    let expected_claim_arguments = structural_arguments
                        .iter()
                        .enumerate()
                        .flat_map(|(argument_index, argument)| {
                            plan.entry_claims
                                .iter()
                                .filter(move |claim| {
                                    argument.byte_sequence_literal().is_none()
                                        && Some(claim.parameter_index)
                                            == argument.source_parameter_index()
                                        && (argument.path.is_empty() || claim.path == argument.path)
                                })
                                .map(move |_| {
                                    u32::try_from(argument_index).map_err(|_| {
                                        LoweringError::Unsupported(
                                            "boundary structural argument index exceeds u32",
                                        )
                                    })
                                })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    validate_transfer_shape(
                        structural_arguments,
                        completion_receipts,
                        parameters,
                        &[],
                        &structural_result_places,
                        &target.structural_parameters,
                        &type_ids,
                        &structural_types,
                        &expected_claim_arguments,
                        &primitive_local_places,
                        None,
                    )?;
                    let (_, boundary, _, target_scalar_parameters) = lowered_boundary_parameters
                        .iter()
                        .find(|(symbol, _, _, _)| *symbol == *target_machine)
                        .ok_or(LoweringError::Unsupported(
                            "boundary structural call target is absent from the lowered closure",
                        ))?;
                    if scalar_arguments.len() != target_scalar_parameters.len() {
                        return unsupported(
                            "boundary structural argument count disagrees with its declaration",
                        );
                    }
                    let arguments = argument_evaluation::validated_values(
                        evaluated_scalar_arguments.as_deref(),
                        target_scalar_parameters,
                    )?
                    .iter()
                    .map(|value| value.id)
                    .collect();
                    let call_byte_places = byte_subslices::argument_places(
                        structural_arguments,
                        &literal_places,
                        &mut next_literal_argument,
                        &staged_subslices[operation_index],
                    )?;
                    let kind = OperationKind::BoundaryCall {
                        boundary: *boundary,
                        arguments,
                        structural_arguments: lower_structural_arguments(
                            structural_arguments,
                            parameters,
                            &[],
                            &structural_result_places,
                            &call_byte_places,
                            &primitive_local_places,
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
                    };
                    let id = operations.allocate();
                    let structural_type = lookup_type_id(&type_ids, type_identity)?;
                    let result_place = place_id(allocate_dense(&mut next_place)?);
                    let result_declaration = StructuralPlaceDeclaration {
                        id: result_place,
                        kind: StructuralPlaceKind::OperationResult {
                            producer: id,
                            structural_type,
                        },
                    };
                    operations.record_source_call(
                        SourceCallCoordinate {
                            state: plan.state,
                            statement_index: usize::try_from(coordinate.statement_index).map_err(
                                |_| {
                                    LoweringError::Unsupported(
                                        "boundary structural call statement coordinate exceeds usize",
                                    )
                                },
                            )?,
                            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                                LoweringError::Unsupported(
                                    "boundary structural call ordinal coordinate exceeds usize",
                                )
                            })?,
                        },
                        *source_site,
                        id,
                        *target_machine,
                    )?;
                    operations.push(Operation {
                        static_reach_binding: None,
                        id,
                        result: OperationResult::Structural(StructuralOperationResult {
                            place: result_place,
                            structural_type,
                            multiplicity: match multiplicity {
                                Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                                Multiplicity::Affine => StructuralMultiplicity::Affine,
                                Multiplicity::Linear => StructuralMultiplicity::Linear,
                            },
                            qualifications: qualifications
                                .iter()
                                .map(|domain| lookup_domain_id(&domain_ids, *domain))
                                .collect::<Result<Vec<_>, _>>()?,
                            projected_qualifications: Vec::new(),
                            claims: Vec::new(),
                        }),
                        kind,
                    });
                    structural_result_places.push((result_declaration, *discard_result_on_return));
                    continue;
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
                CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    statement_index,
                    destination,
                    path,
                    value,
                } => {
                    let destination = match destination {
                        checked_trees::CheckedPrimitiveStoreDestination::Parameter {
                            parameter_index,
                        } => {
                            let parameter = parameters.get(*parameter_index as usize).ok_or(
                                LoweringError::Unsupported("primitive store parameter is absent"),
                            )?;
                            crate::emission::primitive_store::parameter_destination(
                                parameter,
                                path,
                                &structural_types,
                            )?
                        }
                        checked_trees::CheckedPrimitiveStoreDestination::Local { symbol } => {
                            if !path.is_empty() {
                                return unsupported(
                                    "primitive local store has a projected destination",
                                );
                            }
                            let local = primitive_locals::find(&primitive_local_places, *symbol)?;
                            crate::emission::primitive_store::Destination {
                                place: local.declaration.id,
                                path: Vec::new(),
                                scalar_type: local.scalar_type,
                            }
                        }
                    };
                    let kind = crate::emission::primitive_store::emit_assignment(
                        checked,
                        plan.machine,
                        plan.state,
                        *statement_index,
                        destination,
                        value,
                        &mut evaluation,
                        source_value_count,
                        &mut scalar_result_values,
                        &mut next_value_identity,
                        &mut next_block,
                        &mut next_edge,
                        &mut operations,
                        &mut scalar_calls,
                    )?;
                    next_call_obligation = scalar_calls.next_obligation_identity;
                    kind
                }
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) => {
                    crate::emission::structural_byte_sequence_store::literal_view_type(
                        &mut structural_types,
                    )?;
                    crate::emission::structural_byte_sequence_store::emit(
                        store,
                        parameters,
                        &structural_types,
                        &mut literal_places,
                        &mut next_place,
                        &mut next_value_identity,
                        &mut next_call_obligation,
                        &mut operations,
                    )?
                }
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
                    let bindings = crate::expression_preparation::bindings::ScalarBindings::new(
                        scalar_result_values.len(),
                    )
                    .with_primitive_storage(&evaluation.primitive_storage)
                    .with_structural_parameters(&evaluation.structural_parameters)
                    .with_resolved_structural_observations(
                        &evaluation.structural_fields,
                        &evaluation.structural_cases,
                    );
                    let index = bindings.expression_at(
                        checked,
                        plan.state,
                        store.statement_index,
                        CheckedScalarExpressionRole::AssignmentIndex,
                    )?;
                    let value = bindings.expression_at(
                        checked,
                        plan.state,
                        store.statement_index,
                        CheckedScalarExpressionRole::AssignmentValue,
                    )?;
                    crate::emission::structural_byte_sequence_index_store::emit(
                        store,
                        parameters,
                        &structural_types,
                        &index,
                        &value,
                        &scalar_result_values,
                        &mut next_value_identity,
                        &mut next_call_obligation,
                        &mut operations,
                    )?
                }
                CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) => {
                    let bindings = crate::expression_preparation::bindings::ScalarBindings::new(
                        scalar_result_values.len(),
                    )
                    .with_primitive_storage(&evaluation.primitive_storage)
                    .with_structural_parameters(&evaluation.structural_parameters);
                    let index = bindings.expression_at(
                        checked,
                        plan.state,
                        write.statement_index,
                        CheckedScalarExpressionRole::AssignmentIndex,
                    )?;
                    let value = bindings.expression_at(
                        checked,
                        plan.state,
                        write.statement_index,
                        CheckedScalarExpressionRole::AssignmentValue,
                    )?;
                    crate::emission::byte_sequence_write::emit(
                        write,
                        parameters,
                        &structural_types,
                        &index,
                        &value,
                        &scalar_result_values,
                        &mut next_value_identity,
                        &mut next_call_obligation,
                        &mut operations,
                    )?
                }
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
                    let destination = parameters
                        .iter()
                        .find(|parameter| {
                            Some(parameter.position) == store.destination.parameter_position()
                        })
                        .ok_or(LoweringError::Unsupported(
                            "structural scalar store names an unknown parameter",
                        ))?;
                    let lowered =
                        crate::emission::structural_scalar_store::lower_structural_scalar_store_place(
                            store,
                            store.statement_index,
                            destination,
                            &structural_types,
                            crate::emission::structural_scalar_store::StoreAccessPolicy::Exclusive,
                        )?;
                    let value = evaluation.field_assignment_value(
                        checked,
                        plan.machine,
                        plan.state,
                        store,
                        &mut scalar_result_values,
                        &mut next_value_identity,
                        &mut next_block,
                        &mut next_edge,
                        &mut operations,
                        &mut scalar_calls,
                    )?;
                    next_call_obligation = scalar_calls.next_obligation_identity;
                    if value.scalar_type != lowered.scalar_type {
                        return unsupported(
                            "structural scalar store RHS differs from its field type",
                        );
                    }
                    lowered.into_operation(
                        destination.place,
                        value.id,
                        &mut next_call_obligation,
                    )?
                }
                CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
                | CheckedUnitEffectOperationPlan::Complete { .. } => {
                    return unsupported("Unit return is not the final checked operation");
                }
            };
            let id = operations.allocate();
            if let Some((coordinate, source_site, target_machine)) = source_call {
                operations.record_source_call(
                    SourceCallCoordinate {
                        state: plan.state,
                        statement_index: usize::try_from(coordinate.statement_index).map_err(
                            |_| {
                                LoweringError::Unsupported(
                                    "boundary Unit call statement coordinate exceeds usize",
                                )
                            },
                        )?,
                        call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                            LoweringError::Unsupported(
                                "boundary Unit call ordinal coordinate exceeds usize",
                            )
                        })?,
                    },
                    source_site,
                    id,
                    target_machine,
                )?;
            }
            operations.push(Operation {
                static_reach_binding: None,
                id,
                result: terminal_psi::OperationResult::Unit,
                kind,
            });
        }
        if next_literal_argument != call_literal_count {
            return unsupported("byte-sequence literal argument consumption is incomplete");
        }
        let controlled_result = if plan.scalar_control.is_some() {
            // Prefix values and structural locals are already established. The
            // completion adds only the selected guard/arm evaluation and join.
            let mut scalar_calls = CallEmissionContext {
                machine_ids: &machine_ids,
                requirement_counts: &scalar_requirement_counts,
                next_obligation_identity: next_call_obligation,
                obligation_limit: u64::MAX,
            };
            let result = evaluation.scalar_control_result(
                checked,
                plan,
                &mut scalar_result_values,
                &mut next_value_identity,
                &mut next_block,
                &mut next_edge,
                &mut operations,
                &mut scalar_calls,
            )?;
            next_call_obligation = scalar_calls.next_obligation_identity;
            Some(result)
        } else {
            None
        };
        next_operation = operations.next_identity;
        let CheckedUnitEffectOperationPlan::Complete {
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
            ..
        } = plan.operations.last().expect("Unit sequence was validated")
        else {
            unreachable!()
        };
        let local_discards = trivial_affine_local_discard_ordinals.iter().map(|ordinal| {
            local_places
                .get(usize::try_from(*ordinal).map_err(|_| {
                    LoweringError::Unsupported("Unit local cleanup ordinal exceeds usize")
                })?)
                .map(|local| local.id)
                .ok_or(LoweringError::Unsupported(
                    "Unit local cleanup ordinal is not dense",
                ))
        });
        // Legacy locals are a leading constructor prefix; operation results
        // therefore die before those locals, and all locals before parameters.
        // Admitting interleaved legacy constructors requires one declaration-
        // ordered cleanup roster instead of concatenating these two groups.
        // Parameter selection sources have no result row: they follow the
        // result rows in descending authored position so the splice replaces
        // each row with its residual join parameter.
        let mut selection_roster = structural_result_places
            .iter()
            .rev()
            .map(|(place, discard)| (place.id, *discard))
            .collect::<Vec<_>>();
        let mut parameter_sources = Vec::new();
        for cleanup in &evaluation.selection_cleanups {
            for source in &cleanup.sources {
                if let Some((position, _)) = evaluation
                    .structural_parameters
                    .iter()
                    .find(|(_, declaration)| declaration.place == *source)
                {
                    parameter_sources.push((*position, *source));
                }
            }
        }
        parameter_sources.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
        parameter_sources.dedup_by_key(|(_, place)| *place);
        selection_roster.extend(
            parameter_sources
                .into_iter()
                .map(|(_, place)| (place, false)),
        );
        let trivial_affine_discards = evaluation
            .selection_return_discards(selection_roster)?
            .into_iter()
            .map(Ok)
            .chain(local_discards)
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
        let block = evaluation.current;
        let mut scalar_return = plan
            .scalar_result
            .as_ref()
            .map(|result| {
                let position = scalar_parameter_count
                    .checked_add(result.binding_ordinal as usize)
                    .ok_or(LoweringError::Unsupported(
                        "scalar completion binding position overflows",
                    ))?;
                let source =
                    scalar_result_values
                        .get(position)
                        .ok_or(LoweringError::Unsupported(
                            "scalar completion binding is absent",
                        ))?;
                let scalar_type = terminal_scalar_type(result.primitive_type)?;
                if source.scalar_type != scalar_type {
                    return unsupported("scalar completion binding has a different carrier");
                }
                Ok::<_, LoweringError>((
                    source.id,
                    ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(allocate_dense(&mut next_value_identity)?),
                        scalar_type,
                    },
                ))
            })
            .transpose()?;
        if let Some(source) = controlled_result {
            scalar_return = Some((
                source.id,
                ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(allocate_dense(&mut next_value_identity)?),
                    scalar_type: source.scalar_type,
                },
            ));
        }
        // Completion reserves its own scalar result identity after the body.
        // Include it before the next helper borrows the shared value allocator.
        next_value = next_value_identity;
        let structural_return = plan
            .structural_result
            .as_ref()
            .map(|result| {
                let source = match result.source {
                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => {
                        structural_result_places
                            .get(binding_ordinal as usize)
                            .ok_or(LoweringError::Unsupported(
                                "returned structural binding is absent",
                            ))?
                            .0
                            .id
                    }
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                        parameters
                            .get(parameter_index as usize)
                            .ok_or(LoweringError::Unsupported(
                                "returned structural parameter is absent",
                            ))?
                            .place
                    }
                    _ => {
                        return unsupported("structural return source is not an owned whole value");
                    }
                };
                let source = evaluation.current_structural_place(source);
                let place = place_id(allocate_dense(&mut next_place)?);
                let structural_type = lookup_type_id(&type_ids, &result.type_identity)?;
                Ok::<_, LoweringError>((
                    source,
                    terminal_psi::StructuralResultDeclaration {
                        place,
                        structural_type,
                        multiplicity: match result.multiplicity {
                            Multiplicity::Affine => StructuralMultiplicity::Affine,
                            Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                            Multiplicity::Linear => {
                                return unsupported(
                                    "structural completion requires retained linear result claims",
                                );
                            }
                        },
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        reference_sources: result
                            .reference_sources
                            .iter()
                            .map(|reference| {
                                let arguments = parameters::lower_structural_arguments(
                                    std::slice::from_ref(&reference.source),
                                    parameters,
                                    &[],
                                    &[],
                                    &[],
                                    &[],
                                )?;
                                Ok(terminal_psi::StructuralReferenceResultSource {
                                    path: lower_structural_path(&reference.path),
                                    source: arguments[0].clone(),
                                })
                            })
                            .collect::<Result<Vec<_>, LoweringError>>()
                            .map(|mut sources| {
                                // Wire maps use canonical path order, independently
                                // of authored construction and reverse cleanup order.
                                sources.sort_by(|left, right| left.path.cmp(&right.path));
                                sources
                            })?,
                    },
                ))
            })
            .transpose()?;
        let edge = edge_id(allocate_dense(&mut next_edge)?);
        let crash_routes =
            if let Some(contract_plan) = checked.facts.contract_plans.for_machine(plan.machine) {
                if parameters.is_empty() {
                    lower_checked_crash_route_buckets(
                        contract_plan.crash.published(),
                        &scalar_parameters,
                    )?
                } else {
                    lower_structural_crash_route_buckets(
                        contract_plan.crash.published(),
                        &scalar_parameters,
                        &signature.predicate_parameters,
                        &structural_types,
                        runtime_requirements,
                    )?
                }
            } else {
                Vec::new()
            };
        evaluation.remap_transported_call_operands(&mut operations);
        evaluation.blocks.push(Block {
            id: block,
            parameters: evaluation.parameters,
            structural_parameters: evaluation.block_structural_parameters,
            operations: operations[evaluation.operation_start..].to_vec(),
            terminator: if let Some((source, _)) = &scalar_return {
                Terminator::Return {
                    edge,
                    value: *source,
                    cleanup_actions: trivial_affine_discards
                        .into_iter()
                        .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
                        .collect(),
                }
            } else if let Some((source, _)) = &structural_return {
                Terminator::ReturnStructural {
                    edge,
                    source: *source,
                    returned_claims: Vec::new(),
                    trivial_affine_discards,
                }
            } else {
                Terminator::ReturnUnit {
                    edge,
                    trivial_affine_discards,
                }
            },
        });
        evaluation.blocks.sort_by_key(|block| block.id);
        let computed_array_places = evaluation
            .blocks
            .iter()
            .flat_map(|block| {
                crate::scalar_graph::scalar_computations::arrays::declarations(&block.operations)
                    .chain(
                        crate::scalar_graph::scalar_computations::cases::declarations(
                            &block.operations,
                        ),
                    )
            })
            .filter(|place| {
                !structural_result_places
                    .iter()
                    .any(|(existing, _)| existing.id == place.id)
                    && !structural_value_temporaries
                        .iter()
                        .any(|existing| existing.id == place.id)
            })
            .collect::<Vec<_>>();
        let OperationBuffer {
            source_calls,
            selected_ieee_float_fmas,
            selected_ieee_float_comparisons,
            ..
        } = operations;
        source_call_occurrences.extend(source_calls);
        selected_ieee_float_fma_occurrences.extend(selected_ieee_float_fmas);
        selected_ieee_float_comparison_occurrences.extend(selected_ieee_float_comparisons);
        let mut structural_places = parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
            })
            .chain(provider_places.iter().copied())
            .chain(local_places.iter().copied())
            .chain(primitive_local_places.iter().map(|local| local.declaration))
            .chain(literal_places.iter().copied())
            .chain(subslice_places.iter().copied())
            .chain(structural_result_places.iter().map(|(place, _)| *place))
            .chain(structural_value_temporaries)
            .chain(computed_array_places)
            .collect::<Vec<_>>();
        // Argument-time view producers interleave with reserved call results.
        // Declaration order is canonical identity order, not execution order.
        structural_places.sort_by_key(|place| place.id);
        if let Some((_, result)) = &structural_return {
            structural_places.push(StructuralPlaceDeclaration {
                id: result.place,
                kind: StructuralPlaceKind::Result,
            });
        }
        let normal_guarantees = if let Some((_, result)) = scalar_return {
            // Contract result identity is distinct from the body's returned
            // binding. Only exit reconstruction equates them, after normal
            // completion; crashes and incomplete calls establish neither.
            let mut namespace = scalar_parameters.clone();
            namespace.push(result);
            let contract = checked
                .facts
                .contract_plans
                .for_machine(plan.machine)
                .ok_or(LoweringError::Unsupported(
                    "scalar completion has no checked contract",
                ))?;
            let refined = crate::scalar_graph::scalar_contracts::with_result_range(
                checked,
                plan.state,
                scalar_parameters.len(),
                &contract.closed_scalar_values,
            )?;
            crate::scalar_graph::scalar_contracts::clauses(refined.ensures(), &namespace)?
                .into_iter()
                .map(|proposition| {
                    Ok(ContractClause {
                        obligation: obligation_id(allocate_dense(&mut next_call_obligation)?),
                        proposition,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?
        } else {
            Vec::new()
        };
        machines.push(TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: lower_declared_service_reach(
                checked,
                plan.machine,
                &service_ids,
            )?,
            id: terminal_machine,
            attachment,
            parameters: scalar_parameters.clone(),
            structural_parameters: parameters.clone(),
            ranked_scc: None,
            result: if let Some((_, result)) = scalar_return {
                TerminalMachineResult::Scalar(result)
            } else {
                structural_return.map_or(TerminalMachineResult::Unit, |(_, result)| {
                    TerminalMachineResult::Structural(result)
                })
            },
            structural_places,
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
            entry: evaluation.entry,
            blocks: evaluation.blocks,
            contract: MachineContract {
                id: contract_id(terminal_machine.get()),
                crash_routes,
                requires: runtime_requirements.clone(),
                ensures: normal_guarantees,
                outcome_specific_ensures: Vec::new(),
            },
        });
    }

    let mut scalar_evidence = Vec::new();
    for (index, machine) in prepared_scalar_machines.into_iter().enumerate() {
        let terminal_machine = lookup_machine_id(&machine_ids, machine.source_machine())?;
        let machine_index = closure
            .len()
            .checked_add(reserved_prefix)
            .ok_or(LoweringError::Unsupported(
                "shared Unit reserved machine count overflows usize",
            ))?
            .checked_add(index)
            .ok_or(LoweringError::Unsupported(
                "selected scalar closure machine count overflows usize",
            ))?;
        let identity_base = u64::try_from(machine_index)
            .map_err(|_| {
                LoweringError::Unsupported("selected scalar closure machine count exceeds u64")
            })?
            .checked_mul(TERMINAL_MACHINE_IDENTITY_STRIDE)
            .ok_or(LoweringError::Unsupported(
                "selected scalar closure identity range overflows",
            ))?;
        if let PreparedScalarCallee::Structural { plan, .. } = &machine {
            let mut lowered = crate::returns::structural_scalar_return::lower_structural_scalar_return_machine_in_namespace(
                checked, plan, terminal_machine, identity_base, Some(&structural_types),
            )?;
            machines.append(&mut lowered.semantic_module.machines);
            scalar_evidence.append(&mut lowered.proof_bundle.evidence);
            source_call_occurrences.append(&mut lowered.source_call_occurrences);
            selected_ieee_float_fma_occurrences
                .append(&mut lowered.selected_ieee_float_fma_occurrences);
            selected_ieee_float_comparison_occurrences
                .append(&mut lowered.selected_ieee_float_comparison_occurrences);
            continue;
        }
        let PreparedScalarCallee::Graph(machine) = machine else {
            let PreparedScalarCallee::Boundary { plan, .. } = machine else {
                unreachable!("scalar callee has exactly one checked body owner")
            };
            let CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } =
                &plan.boundary_call
            else {
                return unsupported("scalar wrapper lost its boundary operation");
            };
            let boundary = lowered_boundary_parameters
                .iter()
                .find_map(|(source, identity, _, _)| {
                    (*source == *target_machine).then_some(*identity)
                })
                .ok_or(LoweringError::Unsupported(
                    "scalar wrapper boundary is absent from the shared catalog",
                ))?;
            let mut context = CallEmissionContext {
                machine_ids: &machine_ids,
                requirement_counts: &scalar_requirement_counts,
                next_obligation_identity: next_call_obligation,
                obligation_limit: u64::MAX,
            };
            let mut emitted = crate::returns::boundary_scalar_return::emit_boundary_scalar_return(
                checked,
                plan,
                lower_unit_parameters(
                    &plan.structural_parameters,
                    &type_ids,
                    &domain_ids,
                    &mut next_place,
                )?,
                crate::returns::boundary_scalar_return::BoundaryScalarReturnCatalogs {
                    structural_types: &structural_types,
                    type_ids: &type_ids,
                    service_ids: &service_ids,
                },
                crate::returns::boundary_scalar_return::BoundaryScalarReturnIdentities {
                    machine: terminal_machine,
                    contract: contract_id(terminal_machine.get()),
                    boundary,
                    identity_base,
                },
                &mut context,
            )?;
            next_call_obligation = context.next_obligation_identity;
            machines.push(emitted.machine);
            source_call_occurrences.append(&mut emitted.source_call_occurrences);
            selected_ieee_float_fma_occurrences
                .append(&mut emitted.selected_ieee_float_fma_occurrences);
            selected_ieee_float_comparison_occurrences
                .append(&mut emitted.selected_ieee_float_comparison_occurrences);
            continue;
        };
        let graph_parameters = scalar_graph_parameters
            .iter()
            .find(|(source, _, _)| *source == machine.source_machine)
            .ok_or(LoweringError::Unsupported(
                "scalar graph emission lost its allocated parameters",
            ))?;
        if !machine.scalar_qualifications.domains.is_empty() {
            return unsupported(
                "attached scalar graph requires the enclosing qualification catalog namespace",
            );
        }
        let mut lowered =
            crate::scalar_graph::scalar_graph_module::build_scalar_graph_module_in_namespace(
                &machine.states,
                machine.result_type,
                &machine.scalar_qualifications,
                machine.contract,
                machine.crash_routes,
                machine.identity_reshuffles,
                machine.partition_compositions,
                terminal_machine,
                identity_base,
                &machine_ids,
                &scalar_requirement_counts,
                &graph_parameters.1,
                machine.loop_plan.as_ref(),
            )?;
        let [terminal_machine] = lowered.semantic_module.machines.as_mut_slice() else {
            unreachable!("one prepared selected scalar graph emits one terminal machine")
        };
        let reach = checked
            .facts
            .service_reaches
            .for_machine(machine.source_machine)
            .ok_or(LoweringError::Unsupported(
                "scalar graph lost its checked service contract",
            ))?;
        let contract = checked
            .facts
            .service_reaches
            .plan_for_machine(machine.source_machine)
            .ok_or(LoweringError::Unsupported(
                "scalar graph lost its checked service plan",
            ))?;
        terminal_machine.published_service_ceiling = lower_installation_machine_service_ceiling(
            checked,
            machine.source_machine,
            contract,
            ServiceReachSummary {
                direct: reach.inferred_direct,
                transitive: reach.inferred_transitive,
            },
            &service_ids,
        )?;
        terminal_machine.declared_service_reach =
            lower_declared_service_reach(checked, machine.source_machine, &service_ids)?;
        machines.push(terminal_machine.clone());
        scalar_evidence.append(&mut lowered.proof_bundle.evidence);
        source_call_occurrences.append(&mut lowered.source_call_occurrences);
        selected_ieee_float_fma_occurrences
            .append(&mut lowered.selected_ieee_float_fma_occurrences);
        selected_ieee_float_comparison_occurrences
            .append(&mut lowered.selected_ieee_float_comparison_occurrences);
    }

    let mut lowered_structural_realizations = lower_selected_structural_scalar_realizations(
        checked,
        &selected_structural_scalar_roots,
        &structural_types,
        &machine_ids,
        reserved_prefix + closure.len() + scalar_closure.len(),
    )?;
    machines.append(&mut lowered_structural_realizations.machines);
    call_evidence.append(&mut lowered_structural_realizations.evidence);
    source_call_occurrences.append(&mut lowered_structural_realizations.source_calls);

    let mut lowered_structural_result_realizations =
        crate::returns::affine_return::lower_claim_free_affine_return_machines(
            checked,
            &structural_result_roots,
            &structural_types,
            &type_ids,
            &machine_ids,
            reserved_prefix
                + closure.len()
                + scalar_closure.len()
                + selected_structural_scalar_roots.len(),
        )?;
    machines.append(&mut lowered_structural_result_realizations);

    let mut provider_candidates = provider_candidate_plans
        .iter()
        .map(|candidate| {
            let (_, boundary, parameters, scalar_parameters) = lowered_boundary_parameters
                .iter()
                .find(|(symbol, _, _, _)| *symbol == candidate.boundary)
                .ok_or(LoweringError::Unsupported(
                    "provider candidate references an unlowered Unit boundary requirement",
                ))?;
            let terminal_candidate = lookup_machine_id(&machine_ids, candidate.candidate)?;
            let realized = machines
                .iter()
                .find(|machine| machine.id == terminal_candidate)
                .expect("provider candidate root was lowered as an ordinary terminal machine");
            if scalar_parameters.len() != realized.parameters.len()
                || scalar_parameters
                    .iter()
                    .zip(&realized.parameters)
                    .any(|(boundary, candidate)| *boundary != candidate.scalar_type)
            {
                return unsupported(
                    "provider candidate scalar signature disagrees with its boundary requirement",
                );
            }
            Ok(ProviderCandidateConformance {
                boundary: *boundary,
                requirement_identity: candidate.requirement_identity.clone(),
                provider_identity: candidate.provider_identity.clone(),
                candidate_identity: candidate.candidate_identity.clone(),
                candidate: terminal_candidate,
                signature: ProviderSignature {
                    parameters: parameters
                        .iter()
                        .map(|parameter| ProviderSignatureParameter {
                            position: parameter.position,
                            is_self: parameter.is_self,
                            structural_type: parameter.structural_type,
                            multiplicity: parameter.multiplicity,
                            access: parameter.access,
                            qualifications: parameter.qualifications.clone(),
                            projected_qualifications: parameter.projected_qualifications.clone(),
                        })
                        .collect(),
                },
                refinement: ProviderRefinement {
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

    call_evidence.append(&mut scalar_evidence);
    let lowered = LoweredPsi {
        semantic_module: TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            // Operation bodies are emitted first; a scalar entry may follow
            // its helpers. Preserve the selected source owner, not roster order.
            entry: lookup_machine_id(&machine_ids, entry)?,
            structural_types,
            structural_domains,
            services,
            root_service_reach,
            placed_view_inputs,
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines,
            provider_candidates,
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
            machines,
        },
        proof_bundle: ProofBundle {
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: call_evidence,
        },
        debug_map: None,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences,
        selected_ieee_float_comparison_occurrences,
    };
    Ok(shared_closure::SharedUnitClosure {
        lowered,
        machine_ids,
        type_ids,
        domain_ids,
        service_ids,
        boundary_parameters: lowered_boundary_parameters,
        scalar_requirement_counts,
        next_place,
        next_value,
        next_block,
        next_operation,
        next_edge,
        next_call_obligation,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedUnitProviderCandidate {
    body: ProviderBody,
    boundary: symbols::SymbolHandle,
    candidate: symbols::SymbolHandle,
    requirement_identity: String,
    provider_identity: String,
    candidate_identity: String,
}

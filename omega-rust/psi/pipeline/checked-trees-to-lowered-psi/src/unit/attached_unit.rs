//! Attached Unit closure assembly.
//!
//! The orchestrator retains exact transitive closure and publication order;
//! `call_catalog` closes operation/scalar/provider dependencies before allocation;
//! `admission` retains checked bodies and validates source/call custody;
//! `signatures` allocates each machine's formals, claims and requirements.
//! Emission borrows those records in the single shared namespace:
//! `ordinary_machine` emits each ordinary body and `composed_control::callable`
//! each composed one.
use super::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, ClaimTransfer, CompletionReceipt, LoweredPsi, LoweringError,
    MachineContract, Multiplicity, Operation, OperationKind, OperationResult, PlaceId, ProofBundle,
    ProviderCandidateConformance, ProviderParameterRefinement, ProviderRefinement,
    ProviderSignature, ProviderSignatureParameter, ScalarQualificationCatalog, ScalarType,
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
mod ordinary_machine;
mod parameters;
pub(crate) mod primitive_locals;
mod provider_attachments;
mod providers;
mod reference_results;
pub(crate) mod scalar_arrays;
mod scalar_boundaries;
mod scalar_completion;
pub(crate) use scalar_completion::control::validate_tail as validate_scalar_control_tail;
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
use catalog::{lower_program_local_root_introductions, require_valid_service_row};
pub(crate) use composed_control::dynamic_result::{
    emit_call_leaf as emit_dynamic_control_leaf,
    lower_control_catalogs as lower_dynamic_control_catalogs,
};
pub(crate) use composed_control::lower_composed_unit_control_machine;
#[cfg(test)]
pub(crate) use parameters::lower_contract_service_ceiling;
pub(crate) use parameters::{
    checked_scalar_source_parameters, literal_argument_places, lower_boundary_parameter_order,
    lower_installation_machine_service_ceiling, lower_published_service_ceiling,
    lower_structural_arguments, lower_unit_parameters, structural_carrier_type,
    validate_transfer_shape,
};
pub(crate) use provider_attachments::lower_provider_attachment_places;
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
            parameter_order: lower_boundary_parameter_order(
                &plan.scalar_parameters,
                &plan.structural_parameters,
            )?,
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
                .map(|_| (signature.source, signature.requires.len()))
        }))
        .collect::<Vec<_>>();
    let mut scalar_block_invariants = Vec::new();
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
    let mut selected_integer_comparison_occurrences = Vec::new();

    let catalog = ordinary_machine::ClosureCatalog {
        type_ids: &type_ids,
        domain_ids: &domain_ids,
        service_ids: &service_ids,
        boundary_parameters: &lowered_boundary_parameters,
        machine_ids: &machine_ids,
        signatures: &machine_signatures,
        scalar_requirement_counts: &scalar_requirement_counts,
        plans,
        closure: &closure,
        provider_candidate_plans: &provider_candidate_plans,
        prepared_scalar_machines: &prepared_scalar_machines,
    };
    for (signature, admitted) in machine_signatures.iter().zip(admitted_bodies) {
        let body = admitted.source();
        let plan = body.entry()?;
        let terminal_machine = lookup_machine_id(&machine_ids, plan.machine)?;
        let parameters = &signature.parameters;
        let scalar_parameters = signature.scalar_parameters.clone();
        if let admission::AdmittedBody::Composed {
            source: source_plan,
            body: admitted,
        } = admitted
        {
            let (machine, mut occurrences, mut invariants) = composed_control::callable::emit(
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
            scalar_block_invariants.append(&mut invariants);
            continue;
        }
        let plan = body.ordinary()?;
        let emitted = ordinary_machine::emit(
            checked,
            signature,
            plan,
            terminal_machine,
            &mut structural_types,
            catalog,
            composed_control::callable::EmissionCounters {
                place: &mut next_place,
                value: &mut next_value,
                block: &mut next_block,
                operation: &mut next_operation,
                edge: &mut next_edge,
                call_obligation: &mut next_call_obligation,
            },
        )?;
        machines.push(emitted.machine);
        source_call_occurrences.extend(emitted.source_calls);
        selected_ieee_float_fma_occurrences.extend(emitted.selected_ieee_float_fmas);
        selected_ieee_float_comparison_occurrences.extend(emitted.selected_ieee_float_comparisons);
        selected_integer_comparison_occurrences.extend(emitted.selected_integer_comparisons);
    }

    // A nominal consuming member (a seeded `drop<T>` specialization) emitted
    // the ordinary complete-only body; its return edge still invokes the exact
    // owner-attached `::drop` hook the roster selected for each parameter.
    for nominal in &checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
    {
        if nominal.machine.machine == entry || nominal.machine.attachment_type_identity.is_some() {
            continue;
        }
        let Ok(terminal_member) = lookup_machine_id(&machine_ids, nominal.machine.machine) else {
            continue;
        };
        let member = machines
            .iter_mut()
            .find(|member| member.id == terminal_member)
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup member was not emitted in the terminal closure",
            ))?;
        super::unit_cleanup::patch_nominal_cleanup_member(
            checked,
            nominal,
            member,
            &type_ids,
            &machine_ids,
        )?;
    }

    let mut scalar_evidence = Vec::new();
    // Floating entry ranges are machine-local rows keyed by each emitted
    // helper's own identities; they merge into the assembled catalog without
    // sharing the qualification namespace this path keeps empty.
    let mut float_entry_ranges = Vec::new();
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
            selected_integer_comparison_occurrences
                .append(&mut lowered.selected_integer_comparison_occurrences);
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
            selected_integer_comparison_occurrences
                .append(&mut emitted.selected_integer_comparison_occurrences);
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
        float_entry_ranges.append(
            &mut lowered
                .semantic_module
                .scalar_qualifications
                .float_entry_ranges,
        );
        scalar_evidence.append(&mut lowered.proof_bundle.evidence);
        source_call_occurrences.append(&mut lowered.source_call_occurrences);
        selected_ieee_float_fma_occurrences
            .append(&mut lowered.selected_ieee_float_fma_occurrences);
        selected_ieee_float_comparison_occurrences
            .append(&mut lowered.selected_ieee_float_comparison_occurrences);
        selected_integer_comparison_occurrences
            .append(&mut lowered.selected_integer_comparison_occurrences);
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
    float_entry_ranges.sort_by_key(|range| (range.machine, range.parameter));
    // The verifier reads the roster in exactly (machine, header) order.
    scalar_block_invariants.sort_by_key(|invariant| (invariant.machine, invariant.header));
    let lowered = LoweredPsi {
        semantic_module: TerminalModule {
            scalar_qualifications: ScalarQualificationCatalog {
                float_entry_ranges,
                ..Default::default()
            },
            scalar_block_invariants,
            operation_crash_contracts: Vec::new(),
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
        selected_integer_comparison_occurrences,
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

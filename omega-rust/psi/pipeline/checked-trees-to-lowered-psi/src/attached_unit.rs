//! Attached Unit closure assembly.
//!
//! The orchestrator retains exact transitive closure and publication order;
//! `call_catalog` closes operation/scalar/provider dependencies before allocation;
//! call-closure custody, semantic catalogs, and parameter
//! transfer validation live in separate subordinate modules.

use super::*;
use crate::runtime_requirements::substitute_runtime_requirement_scalar_values;
use crate::scalar_call_closure::callee::{CheckedScalarCallee, PreparedScalarCallee};
use checked_trees::CheckedUnitStructuralArgumentSourcePlan;

pub(crate) mod argument_evaluation;
mod argument_schedule;
pub(crate) mod bodies;
mod byte_subslices;
mod call_catalog;
mod call_closure;
pub(crate) mod catalog;
mod claims;
mod composed_control;
mod parameters;
pub(crate) mod primitive_locals;
mod provider_attachments;
mod providers;
pub(crate) mod scalar_arrays;
mod scalar_boundaries;
mod scalar_completion;
mod scalar_structural_calls;
mod selected_operator;
pub(super) mod shared_closure;
mod structural_calls;

use bodies::UnitBody;
use parameters::lower_unit_scalar_parameter_types;
pub(super) use parameters::validate_direct_unit_parameter_custody;

use call_closure::{
    checked_scalar_call_closure_with_structural_roots, checked_terminal_machine_name,
    reject_recursive_unit_closure, unique_unit_boundary, validate_unit_operation_sequence,
};
pub(super) use call_closure::{
    checked_unit_boundary_identity, checked_unit_call_closure_including, unique_unit_machine,
};
#[cfg(test)]
pub(super) use catalog::collect_contract_services;
pub(super) use catalog::{
    checked_unit_target_reach_matches, collect_installation_machine_contract_services,
    collect_published_contract_services, collect_service_summary, lower_root_service_reach,
    lower_unit_structural_type_roots,
};
use catalog::{
    lower_program_local_root_introductions, lower_provider_candidate_service_ceiling,
    require_valid_service_row,
};
use claims::lower_unit_entry_claims;
pub(super) use composed_control::dynamic_result::{
    emit_call_leaf as emit_dynamic_control_leaf,
    lower_control_catalogs as lower_dynamic_control_catalogs,
};
pub(super) use composed_control::lower_composed_unit_control_machine;
#[cfg(test)]
pub(super) use parameters::lower_contract_service_ceiling;
pub(super) use parameters::{
    checked_scalar_source_parameters, literal_argument_places,
    lower_installation_machine_service_ceiling, lower_published_service_ceiling,
    lower_structural_arguments, lower_structural_path, lower_unit_parameters,
    validate_transfer_shape,
};
pub(super) use provider_attachments::lower_provider_attachment_places;
use provider_attachments::validate_provider_attachment_requirements;
use providers::{ProviderBody, checked_unit_provider_candidates};
use selected_operator::{
    lower_selected_structural_scalar_realizations, validate_selected_operator_scalar_call,
    validate_selected_operator_structural_call, validate_selected_operator_structural_scalar_call,
};

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

pub(crate) fn retain_exact_flow_call(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    target: symbols::SymbolHandle,
) -> Result<&checked_trees::FlowCallFact, LoweringError> {
    let mut states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter_map(|(_, state)| {
            (state.machine_symbol == machine && state.state_symbol == source_state).then_some(state)
        });
    let Some(state) = states.next() else {
        return unsupported("Unit scalar call is missing its original checked flow state");
    };
    if states.next().is_some() {
        return unsupported("Unit scalar call has duplicate original checked flow states");
    }
    let statement_index = usize::try_from(coordinate.statement_index).map_err(|_| {
        LoweringError::Unsupported("Unit scalar call statement coordinate exceeds usize")
    })?;
    let call_ordinal = usize::try_from(coordinate.call_ordinal).map_err(|_| {
        LoweringError::Unsupported("Unit scalar call ordinal coordinate exceeds usize")
    })?;
    let authored =
        crate::call_source_custody::authored::locate_source(checked, source_state, coordinate)?;
    if authored.target_state != target {
        return unsupported("Unit result call disagrees with its authored resolved target");
    }
    let mut exact_calls = checked
        .facts
        .flow
        .control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .filter(|call| {
            call.statement_index == statement_index && call.call_ordinal == call_ordinal
        });
    // Flow retains the authored callable parameter, while the operation names
    // its resolved boundary requirement. Rejoin both identities through source.
    let exact = exact_calls.next().ok_or(LoweringError::Unsupported(
        "Unit call has no original checked flow occurrence",
    ))?;
    if exact.target_symbol != authored.source_target || exact_calls.next().is_some() {
        return unsupported(
            "Unit scalar call coordinate and target do not rejoin its original checked flow call",
        );
    }
    Ok(exact)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn retain_exact_unit_boundary<'plans>(
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

pub(super) fn lower_unit_effect_closure(
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
) -> Result<crate::machine_dispatch::SourceMappedLowered, LoweringError> {
    let closure = lower_shared_unit_closure(checked, entry, &[entry], None)?;
    crate::machine_dispatch::SourceMappedLowered::new(closure.lowered, closure.machine_ids)
}

/// Nominal cleanup assembles the final caller and cleanup contracts itself,
/// after restoring their full scalar and structural namespaces.
pub(super) fn lower_nominal_cleanup_closure(
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

pub(super) fn lower_shared_unit_closure(
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
) -> Result<crate::machine_dispatch::SourceMappedLowered, LoweringError> {
    let mut closure = assemble_unit_closure(
        checked,
        entry,
        &[],
        None,
        RuntimeRequirementOwner::UnitClosure,
        true,
    )?;
    finalize_operation_proofs(&mut closure.lowered)?;
    crate::machine_dispatch::SourceMappedLowered::new(closure.lowered, closure.machine_ids)
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
    let scalar_roots = call_catalog.scalar_roots;
    let scalar_closure = call_catalog.scalars;
    let scalar_callees = scalar_closure
        .iter()
        .map(|machine| {
            crate::scalar_call_closure::callee::CheckedScalarCallee::find_for_unit_call(
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
    let mut composed_bodies = Vec::new();
    for machine_symbol in &closure {
        let body = UnitBody::find(plans, *machine_symbol)?;
        if let UnitBody::Composed(plan) = body {
            let admitted = composed_control::admit_callable(checked, plan)?;
            for (boundary, _) in admitted.boundaries() {
                retain_exact_unit_boundary(
                    checked,
                    plans,
                    &mut boundaries,
                    boundary.machine,
                    boundary.state,
                    boundary.contract_report_fingerprint,
                    boundary.service_reach,
                    boundary.result.clone(),
                )?;
            }
            composed_bodies.push((*machine_symbol, admitted));
            continue;
        }
        let machine = body.ordinary()?;
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
        validate_unit_operation_sequence(checked, machine)?;
        // The nominal-cleanup owner validates a synthetic empty completion for
        // its entry, then installs the actual scalar result and full contract.
        // Ordinary entries and every transitive helper retain authored results.
        let synthetic_cleanup_entry = requirements_owner == RuntimeRequirementOwner::NominalCleanup
            && machine.machine == entry
            && machine.structural_result.is_none()
            && machine.scalar_result.is_none()
            && machine.scalar_control.is_none()
            && matches!(machine.operations.as_slice(), [CheckedUnitEffectOperationPlan::Complete { statement_index: 0, trivial_affine_local_discard_ordinals, trivial_affine_discards }] if trivial_affine_local_discard_ordinals.is_empty() && trivial_affine_discards.is_empty());
        if !synthetic_cleanup_entry {
            if machine.scalar_result.is_some() || machine.scalar_control.is_some() {
                scalar_completion::validate(checked, machine)?;
            } else {
                scalar_arrays::validate_result(checked, machine)?;
            }
        }
        crate::structural_scalar_store_source::validate(checked, machine)?;
        crate::call_source_custody::validate_store_and_initializer_calls(checked, machine)?;
        for (operation_index, operation) in machine.operations.iter().enumerate() {
            provider_attachments::validate_call_source(
                checked,
                machine.machine,
                machine.state,
                operation,
                &machine.provider_attachment_requirements,
            )?;
            crate::call_source_custody::validate_operation(
                checked,
                machine.machine,
                machine.state,
                operation,
                &machine.structural_parameters,
            )?;
            match operation {
                CheckedUnitEffectOperationPlan::EstablishScalarArray {
                    source,
                    result,
                    elements,
                } => {
                    scalar_arrays::validate(checked, machine, *source, result, elements)?;
                }
                CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. } => {
                    structural_calls::validate_cleanup(checked, machine, operation_index)?;
                }
                CheckedUnitEffectOperationPlan::StructuralCall { target_machine, .. }
                    if !UnitBody::contains(plans, *target_machine) =>
                {
                    structural_calls::validate(checked, machine, operation)?;
                }
                CheckedUnitEffectOperationPlan::CallUnit {
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    service_reach,
                    ..
                }
                | CheckedUnitEffectOperationPlan::StructuralCall {
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    service_reach,
                    ..
                } => {
                    let body = UnitBody::find(plans, *target_machine)?;
                    structural_calls::validate_body_result(checked, operation, body.result()?)?;
                    let target = body.entry()?;
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
                    primitive_locals::unit_calls::validate(
                        checked,
                        machine,
                        operation,
                        target.structural_parameters,
                    )?;
                    crate::call_source_custody::projected_receivers::validate(
                        checked,
                        machine,
                        operation,
                        target.structural_parameters,
                    )?;
                    structural_calls::validate_consumer(
                        checked,
                        machine,
                        operation,
                        target.structural_parameters,
                        target.entry_claims,
                    )?;
                }
                CheckedUnitEffectOperationPlan::ScalarCall {
                    coordinate,
                    result,
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    target_contract_commitment,
                    service_reach,
                    scalar_arguments,
                    structural_arguments,
                    claim_transfers,
                } => {
                    let source_call = retain_exact_flow_call(
                        checked,
                        machine.machine,
                        machine.state,
                        *coordinate,
                        *target_state,
                    )?;
                    let target = CheckedScalarCallee::find_for_unit_call(checked, *target_machine)?;
                    match &target {
                        CheckedScalarCallee::Boundary(_) | CheckedScalarCallee::Structural(_) => {
                            scalar_structural_calls::validate_call_source(
                                checked, machine, operation, &target,
                            )?
                        }
                        CheckedScalarCallee::Graph(_) | CheckedScalarCallee::Operations(_)
                            if !structural_arguments.is_empty() || !claim_transfers.is_empty() =>
                        {
                            scalar_structural_calls::validate_call_source(
                                checked, machine, operation, &target,
                            )?;
                        }
                        CheckedScalarCallee::Graph(_) | CheckedScalarCallee::Operations(_) => {}
                    }
                    if !scalar_closure.contains(target_machine)
                        && !(closure.contains(target_machine)
                            && matches!(target, CheckedScalarCallee::Operations(_)))
                    {
                        return unsupported(
                            "ordinary Unit scalar call target is absent from the checked closure",
                        );
                    }
                    let contract = checked
                        .facts
                        .contract_plans
                        .for_machine(*target_machine)
                        .ok_or(LoweringError::Unsupported(
                            "ordinary Unit scalar call target has no checked contract",
                        ))?;
                    let target_reaches = checked
                        .facts
                        .flow
                        .control
                        .states
                        .iter()
                        .filter(|(_, state)| {
                            state.machine_symbol == *target_machine
                                && state.state_symbol == *target_state
                        })
                        .map(|(_, state)| state.service_reach)
                        .collect::<Vec<_>>();
                    let reach_matches = match &target {
                        CheckedScalarCallee::Graph(_) | CheckedScalarCallee::Structural(_) => {
                            target_reaches.as_slice() == [*service_reach]
                        }
                        CheckedScalarCallee::Operations(plan) => {
                            // The body owns its direct effects; an ordinary call
                            // contributes the published callee ceiling transitively.
                            // Rejoin each subject instead of equating their summaries;
                            // the caller still retains its exact source occurrence row.
                            source_call.service_reach == *service_reach
                                && target_reaches.as_slice() == [plan.service_reach]
                                && checked
                                    .facts
                                    .service_reaches
                                    .plan_for_machine(*target_machine)
                                    == Some(plan.contract_service_reach)
                                && checked_unit_target_reach_matches(
                                    *service_reach,
                                    plan.contract_service_reach,
                                )
                        }
                        CheckedScalarCallee::Boundary(plan) => {
                            target_reaches.as_slice() == [plan.service_reach]
                                && checked_unit_target_reach_matches(
                                    *service_reach,
                                    plan.contract_service_reach,
                                )
                        }
                    };
                    if target.entry_state()? != *target_state
                        || target.parameter_types()?.len() != scalar_arguments.len()
                        || target.result_type()? != terminal_scalar_type(result.primitive_type)?
                        || contract.report_fingerprint != *target_contract_report_fingerprint
                        || contract.commitment != *target_contract_commitment
                        || !reach_matches
                    {
                        return unsupported(
                            "ordinary Unit scalar call disagrees with its checked target signature, contract, or reach",
                        );
                    }
                    if matches!(
                        target,
                        CheckedScalarCallee::Graph(_) | CheckedScalarCallee::Structural(_)
                    ) && (!checked
                        .facts
                        .service_reaches
                        .rows
                        .services(service_reach.direct)
                        .is_empty()
                        || !checked
                            .facts
                            .service_reaches
                            .rows
                            .services(service_reach.transitive)
                            .is_empty())
                    {
                        return unsupported(
                            "ordinary Unit scalar call with services requires scalar service lowering",
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
                    retain_exact_unit_boundary(
                        checked,
                        plans,
                        &mut boundaries,
                        *target_machine,
                        *target_state,
                        *target_contract_report_fingerprint,
                        *service_reach,
                        CheckedBoundaryMachineResultPlan::Unit,
                    )?;
                }
                CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                    coordinate,
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    service_reach,
                    result,
                    ..
                } => {
                    retain_exact_checked_flow_call(checked, machine, *coordinate, *target_state)?;
                    retain_exact_unit_boundary(
                        checked,
                        plans,
                        &mut boundaries,
                        *target_machine,
                        *target_state,
                        *target_contract_report_fingerprint,
                        *service_reach,
                        CheckedBoundaryMachineResultPlan::Scalar(result.primitive_type),
                    )?;
                }
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    coordinate,
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    service_reach,
                    result,
                    ..
                } => {
                    retain_exact_checked_flow_call(checked, machine, *coordinate, *target_state)?;
                    let target = unique_unit_boundary(plans, *target_machine)?;
                    if !matches!(
                        &target.result,
                        CheckedBoundaryMachineResultPlan::Structural {
                            type_identity,
                            multiplicity,
                            ..
                        } if type_identity == &result.type_identity
                            && multiplicity == &result.multiplicity
                    ) {
                        return unsupported(
                            "Unit structural result drifted from its checked boundary target",
                        );
                    }
                    retain_exact_unit_boundary(
                        checked,
                        plans,
                        &mut boundaries,
                        *target_machine,
                        *target_state,
                        *target_contract_report_fingerprint,
                        *service_reach,
                        target.result.clone(),
                    )?;
                }
                CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                    coordinate,
                    result,
                    requirement_operator,
                    provider_plan_report_fingerprint,
                    provider_plan_commitment,
                    realization_machine,
                    realization_state,
                    realization_contract_report_fingerprint,
                    realization_contract_commitment,
                    service_reach,
                    scalar_arguments,
                    ..
                } => {
                    validate_selected_operator_scalar_call(
                        checked,
                        machine,
                        *coordinate,
                        *result,
                        *requirement_operator,
                        *provider_plan_report_fingerprint,
                        *provider_plan_commitment,
                        *realization_machine,
                        *realization_state,
                        *realization_contract_report_fingerprint,
                        *realization_contract_commitment,
                        *service_reach,
                        scalar_arguments.len(),
                    )?;
                }
                CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                    coordinate,
                    result,
                    requirement_operator,
                    provider_plan_report_fingerprint,
                    provider_plan_commitment,
                    realization_machine,
                    realization_state,
                    realization_contract_report_fingerprint,
                    realization_contract_commitment,
                    service_reach,
                    scalar_arguments,
                    structural_arguments,
                } => {
                    validate_selected_operator_structural_scalar_call(
                        checked,
                        machine,
                        *coordinate,
                        *result,
                        *requirement_operator,
                        *provider_plan_report_fingerprint,
                        *provider_plan_commitment,
                        *realization_machine,
                        *realization_state,
                        *realization_contract_report_fingerprint,
                        *realization_contract_commitment,
                        *service_reach,
                        scalar_arguments,
                        structural_arguments,
                    )?;
                }
                CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                    coordinate,
                    result,
                    requirement_operator,
                    provider_plan_report_fingerprint,
                    provider_plan_commitment,
                    realization_machine,
                    realization_state,
                    realization_contract_report_fingerprint,
                    realization_contract_commitment,
                    service_reach,
                    scalar_arguments,
                    structural_arguments,
                    discard_result_on_return,
                } => {
                    validate_selected_operator_structural_call(
                        checked,
                        machine,
                        *coordinate,
                        result,
                        *requirement_operator,
                        *provider_plan_report_fingerprint,
                        *provider_plan_commitment,
                        *realization_machine,
                        *realization_state,
                        *realization_contract_report_fingerprint,
                        *realization_contract_commitment,
                        *service_reach,
                        scalar_arguments,
                        structural_arguments,
                        *discard_result_on_return,
                    )?;
                }
                CheckedUnitEffectOperationPlan::PortWrite { .. }
                | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd { .. }
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                | CheckedUnitEffectOperationPlan::Complete { .. } => {}
            }
        }
    }
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
            crate::structural_byte_sequence_store::literal_view_type(&mut structural_types)?;
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
        .flat_map(|parameter| &parameter.qualifications)
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
            published_service_ceiling,
        });
        lowered_boundary_parameters.push((plan.machine, id, parameters, scalar_parameters));
    }

    // Allocate the actual formal values before lowering contracts. Calls and
    // bodies share these declarations; requirement terms never use placeholder IDs.
    let mut next_value = 1_u64;
    let mut lowered_machine_parameters = Vec::with_capacity(closure.len());
    let mut lowered_machine_scalar_parameters = Vec::with_capacity(closure.len());
    let mut lowered_claims = Vec::with_capacity(closure.len());
    for machine_symbol in &closure {
        let body = UnitBody::find(plans, *machine_symbol)?;
        let plan = body.entry()?;
        if body.qualifications().iter().any(|domain| {
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
            plan.structural_parameters,
            &type_ids,
            &domain_ids,
            &mut next_place,
        )?;
        let scalar_parameters = lower_unit_scalar_parameter_types(plan.scalar_parameters)?
            .into_iter()
            .map(|scalar_type| {
                Ok(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(allocate_dense(&mut next_value)?),
                    scalar_type,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        // ClaimId is machine-local; unrelated closure members must not shift
        // this machine's canonical claim namespace.
        let claims =
            lower_unit_entry_claims(plan.machine, plan.state, plan.entry_claims, &parameters)?;
        lowered_machine_parameters.push((*machine_symbol, parameters));
        lowered_machine_scalar_parameters.push((*machine_symbol, scalar_parameters));
        lowered_claims.push((*machine_symbol, claims.entry_claims, claims.source_claims));
    }

    // Predicate roots use authored positions; Terminal signatures use dense
    // structural positions. These lowering-only views keep the exact emitted
    // place/type identities without inserting dummy slots for scalar arguments.
    let predicate_parameters = closure
        .iter()
        .zip(&lowered_machine_parameters)
        .map(|(symbol, (lowered_symbol, parameters))| {
            let plan = UnitBody::find(plans, *symbol)?.entry()?;
            if symbol != lowered_symbol || plan.structural_parameters.len() != parameters.len() {
                return unsupported(
                    "Unit predicate signature does not match its structural roster",
                );
            }
            let parameters = plan
                .structural_parameters
                .iter()
                .zip(parameters)
                .map(|(source, terminal)| StructuralParameterDeclaration {
                    position: source.position,
                    ..terminal.clone()
                })
                .collect::<Vec<_>>();
            Ok((*symbol, parameters))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;

    let lowered_machine_runtime_requirements = closure
        .iter()
        .map(|machine_symbol| {
            let Some(contract) = checked.facts.contract_plans.for_machine(*machine_symbol) else {
                return Ok((*machine_symbol, Vec::new()));
            };
            // Entry requirements also justify body operations and ordinary
            // calls. Their presence cannot depend on arithmetic in a published
            // crash ceiling (which may be unconditional or absent).
            let requirements = if (requirements_owner == RuntimeRequirementOwner::UnitClosure
                && contract.crash.structural_runtime_requirements().is_some())
                || contract.crash.uses_structural_proof_gated_arithmetic()
            {
                let checked_requirements = contract.crash.structural_runtime_requirements().ok_or(
                    LoweringError::Unsupported(
                        "proof-gated structural arithmetic lacks a complete checked requirement package",
                    ),
                )?;
                let parameters = predicate_parameters
                    .iter()
                    .find_map(|(symbol, parameters)| {
                        (*symbol == *machine_symbol).then_some(parameters)
                    })
                    .expect("every closure machine has lowered parameters");
                let scalar_parameters = lowered_machine_scalar_parameters
                    .iter()
                    .find_map(|(symbol, parameters)| {
                        (*symbol == *machine_symbol).then_some(parameters)
                    })
                    .expect("every closure machine has lowered scalar parameters");
                let requirements = checked_requirements
                    .iter()
                    .map(|requirement| {
                        lower_structural_runtime_requirement(
                            requirement,
                            scalar_parameters,
                            parameters,
                            &structural_types,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut keyed = requirements
                    .into_iter()
                    .map(|requirement| {
                        terminal_codec::canonical_proposition_order_key(&requirement)
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
                crate::scalar_graph_lowering::primitive_locals::allocate(
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
                scalar_roots.contains(&source) && !(scalar_entry && source == entry),
                &parameters.1,
                &parameters.2,
                &structural_types,
                &mut next_place,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let scalar_requirement_counts =
        prepared_scalar_machines
            .iter()
            .map(|machine| (machine.source_machine(), machine.requirement_count()))
            .chain(lowered_machine_runtime_requirements.iter().filter_map(
                |(source, requirements)| {
                    plans
                        .for_machine(*source)
                        .filter(|plan| {
                            plan.scalar_result.is_some() || plan.scalar_control.is_some()
                        })
                        .map(|_| (*source, requirements.len()))
                },
            ))
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

    for machine_symbol in &closure {
        let body = UnitBody::find(plans, *machine_symbol)?;
        let plan = body.entry()?;
        let terminal_machine = lookup_machine_id(&machine_ids, plan.machine)?;
        let parameters = lowered_machine_parameters
            .iter()
            .find_map(|(symbol, parameters)| (*symbol == plan.machine).then_some(parameters))
            .expect("every closure machine has lowered parameters");
        let scalar_parameters = lowered_machine_scalar_parameters
            .iter()
            .find_map(|(symbol, parameters)| (*symbol == plan.machine).then_some(parameters))
            .expect("every closure machine has lowered scalar parameters")
            .clone();
        let scalar_parameter_count = scalar_parameters.len();
        let runtime_requirements = lowered_machine_runtime_requirements
            .iter()
            .find_map(|(symbol, requirements)| (*symbol == plan.machine).then_some(requirements))
            .expect("every closure machine has lowered runtime requirements");
        let (_, entry_claims, claim_bindings) = lowered_claims
            .iter()
            .find(|(symbol, _, _)| *symbol == plan.machine)
            .expect("every closure machine has lowered entry claims");
        if let UnitBody::Composed(source_plan) = body {
            let position = composed_bodies
                .iter()
                .position(|(source, _)| *source == *machine_symbol)
                .ok_or(LoweringError::Unsupported(
                    "composed callable has no admitted body",
                ))?;
            let (_, admitted) = composed_bodies.remove(position);
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
                    scalar_parameters: &lowered_machine_scalar_parameters,
                    requirements: &lowered_machine_runtime_requirements,
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
        let mut affine_scalar_record_places = Vec::<StructuralPlaceDeclaration>::new();
        let mut primitive_local_places = Vec::<primitive_locals::PrimitiveLocal>::new();
        let mut structural_result_places = Vec::<(StructuralPlaceDeclaration, bool)>::new();
        let mut evaluation = argument_evaluation::Evaluation::new(&mut next_block)?;
        evaluation.arrays = crate::scalar_computations::arrays::prepare(
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
            crate::scalar_source_custody::primitive_references::bindings(
                checked,
                plan.state,
                &evaluation.structural_parameters,
                &structural_types,
            )?;
        let mut staged_arguments = vec![Vec::<usize>::new(); plan.operations.len()];
        evaluation.structural_fields =
            crate::scalar_bindings::StructuralScalarFieldBinding::collect(
                &evaluation.structural_parameters,
                &structural_types,
            );
        evaluation.structural_cases =
            crate::scalar_bindings::structural_cases::StructuralCaseBinding::collect(
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
                    let value = crate::scalar_bindings::ScalarBindings::new(source_value_count)
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
                        scalar_arrays::bind_local(
                            checked,
                            plan,
                            operation,
                            declaration.id,
                            &mut evaluation.array_locals,
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
                CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal {
                    declaration_ordinal,
                    type_identity,
                    field_identity,
                    value,
                    ..
                } => {
                    if usize::try_from(*declaration_ordinal).ok()
                        != Some(affine_scalar_record_places.len())
                    {
                        return unsupported("Unit affine scalar-record local ordinal is not dense");
                    }
                    let structural_type = lookup_type_id(&type_ids, type_identity)?;
                    let declaration = structural_types
                        .iter()
                        .find(|declaration| declaration.id == structural_type)
                        .ok_or(LoweringError::Unsupported(
                            "Unit affine scalar-record local type is absent",
                        ))?;
                    let StructuralTypeShape::Record { fields } = &declaration.shape else {
                        return unsupported(
                            "Unit affine scalar-record local is not a direct record",
                        );
                    };
                    let [field] = fields.as_slice() else {
                        return unsupported(
                            "Unit affine scalar-record local does not have exactly one field",
                        );
                    };
                    let expected_scalar_type = terminal_scalar_type(PrimitiveType::I64)?;
                    if field.identity != *field_identity
                        || field.relevance != terminal_psi::BindingRelevance::Relevant
                        || field.field_type != StructuralFieldType::Scalar(expected_scalar_type)
                    {
                        return unsupported(
                            "Unit affine scalar-record field drifted from checked custody",
                        );
                    }
                    let CheckedScalarExpression::IntegerLiteral { literal } = value else {
                        return unsupported(
                            "Unit affine scalar-record value is not an integer literal",
                        );
                    };
                    if integer_landing_scalar_type(literal)? != expected_scalar_type {
                        return unsupported(
                            "Unit affine scalar-record value is not an exact signed i64",
                        );
                    }
                    let operation_id = operations.allocate();
                    let result_place = place_id(allocate_dense(&mut next_place)?);
                    let result_declaration = StructuralPlaceDeclaration {
                        id: result_place,
                        kind: StructuralPlaceKind::OperationResult {
                            producer: operation_id,
                            structural_type,
                        },
                    };
                    operations.push(Operation {
                        id: operation_id,
                        result: OperationResult::Structural(StructuralOperationResult {
                            place: result_place,
                            structural_type,
                            multiplicity: StructuralMultiplicity::Affine,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            claims: Vec::new(),
                        }),
                        kind: OperationKind::EstablishAffineScalarRecord {
                            field: field.id,
                            value: integer_value(literal, expected_scalar_type)?,
                        },
                    });
                    affine_scalar_record_places.push(result_declaration);
                    continue;
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
                    scalar_arrays::bind_local(
                        checked,
                        plan,
                        operation,
                        result.0.id,
                        &mut evaluation.array_locals,
                    )?;
                    structural_result_places.push(result);
                    continue;
                }
                CheckedUnitEffectOperationPlan::CallUnit {
                    coordinate,
                    target_machine,
                    target_state,
                    scalar_arguments,
                    structural_arguments,
                    ..
                }
                | CheckedUnitEffectOperationPlan::StructuralCall {
                    coordinate,
                    target_machine,
                    target_state,
                    scalar_arguments,
                    structural_arguments,
                    ..
                } => {
                    let claim_transfers = match operation {
                        CheckedUnitEffectOperationPlan::CallUnit {
                            claim_transfers, ..
                        } => claim_transfers.as_slice(),
                        _ => &[],
                    };
                    let target = UnitBody::find(plans, *target_machine)?.entry()?;
                    if scalar_arguments.len() != target.scalar_parameters.len() {
                        return unsupported(
                            "Unit call scalar argument count disagrees with its target",
                        );
                    }
                    let terminal_scalar_values = argument_evaluation::validated_values(
                        evaluated_scalar_arguments.as_deref(),
                        &target
                            .scalar_parameters
                            .iter()
                            .map(|parameter| terminal_scalar_type(parameter.primitive_type))
                            .collect::<Result<Vec<_>, _>>()?,
                    )?;
                    let terminal_scalar_arguments = terminal_scalar_values
                        .iter()
                        .map(|value| value.id)
                        .collect();
                    validate_transfer_shape(
                        structural_arguments,
                        claim_transfers,
                        parameters,
                        &local_places,
                        &affine_scalar_record_places,
                        &structural_result_places,
                        target.structural_parameters,
                        &type_ids,
                        &structural_types,
                        &target
                            .entry_claims
                            .iter()
                            .map(|claim| claim.parameter_index)
                            .collect::<Vec<_>>(),
                        &primitive_local_places,
                    )?;
                    let call_byte_places = byte_subslices::argument_places(
                        structural_arguments,
                        &literal_places,
                        &mut next_literal_argument,
                        &staged_subslices[operation_index],
                    )?;
                    let terminal_arguments = lower_structural_arguments(
                        structural_arguments,
                        parameters,
                        &local_places,
                        &affine_scalar_record_places,
                        &structural_result_places,
                        &call_byte_places,
                        &primitive_local_places,
                    )?;
                    let target_parameters = lowered_machine_parameters
                        .iter()
                        .find_map(|(symbol, parameters)| {
                            (*symbol == *target_machine).then_some(parameters)
                        })
                        .expect("every closure target has lowered parameters");
                    let target_scalar_parameters = lowered_machine_scalar_parameters
                        .iter()
                        .find_map(|(symbol, parameters)| {
                            (*symbol == *target_machine).then_some(parameters)
                        })
                        .expect("every closure target has lowered scalar parameters");
                    if target_scalar_parameters.len() != terminal_scalar_values.len() {
                        return unsupported("Unit call scalar requirement arity is inconsistent");
                    }
                    let scalar_substitutions = target_scalar_parameters
                        .iter()
                        .zip(&terminal_scalar_values)
                        .map(|(formal, actual)| {
                            if formal.scalar_type != actual.scalar_type {
                                return unsupported(
                                    "Unit call scalar requirement parameter type is inconsistent",
                                );
                            }
                            Ok((formal.id, *actual))
                        })
                        .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
                    // Preserve callee requirement slots after substitution, even
                    // if reordered or equal arguments change canonical term order.
                    let target_runtime_requirements = lowered_machine_runtime_requirements
                        .iter()
                        .find_map(|(symbol, requirements)| {
                            (*symbol == *target_machine).then_some(requirements)
                        })
                        .expect("every closure target has lowered runtime requirements")
                        .iter()
                        .map(|requirement| {
                            let mut requirement = requirement.clone();
                            substitute_runtime_requirement_scalar_values(
                                &mut requirement,
                                &scalar_substitutions,
                            )?;
                            Ok(requirement)
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    let mut crash_continuations = if let Some(target_contract) =
                        checked.facts.contract_plans.for_machine(*target_machine)
                    {
                        if target_parameters.is_empty() {
                            lower_checked_crash_route_buckets(
                                target_contract.crash.published(),
                                &terminal_scalar_values,
                            )?
                        } else {
                            lower_structural_crash_route_buckets(
                                target_contract.crash.published(),
                                &terminal_scalar_values,
                                &predicate_parameters
                                    .iter()
                                    .find(|(symbol, _)| *symbol == *target_machine)
                                    .expect("every closure target has predicate parameter bindings")
                                    .1,
                                &structural_types,
                                &target_runtime_requirements,
                            )?
                        }
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
                                    if call_byte_places.contains(&argument.place) {
                                        // Transfer validation already required the exact whole
                                        // immutable byte view. Its canonical path has no segments.
                                        Vec::new()
                                    } else {
                                        structural_crash_route_argument_prefix(
                                            argument,
                                            parameters,
                                            &local_places,
                                            &affine_scalar_record_places,
                                            &structural_result_places,
                                            &structural_types,
                                            &primitive_local_places,
                                        )?
                                    },
                                ),
                            ))
                        })
                        .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
                    substitute_structural_crash_route_roots(
                        &mut crash_continuations,
                        &substitutions,
                    )?;
                    source_call = Some((*coordinate, None, *target_state));
                    let requirement_obligations = target_runtime_requirements
                        .iter()
                        .map(|_| {
                            // Proof finalization reconstructs this exact callee
                            // slot against the completed caller's pre-call facts.
                            let obligation = obligation_id(next_call_obligation);
                            next_call_obligation = next_call_obligation
                                .checked_add(1)
                                .ok_or(LoweringError::Unsupported(
                                    "runtime structural call obligation identity space is exhausted",
                                ))?;
                            Ok(obligation)
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                    if let CheckedUnitEffectOperationPlan::StructuralCall {
                        source_site,
                        result,
                        discard_result_on_return,
                        ..
                    } = operation
                    {
                        let id = operations.allocate();
                        let place = place_id(allocate_dense(&mut next_place)?);
                        let structural_type = lookup_type_id(&type_ids, &result.type_identity)?;
                        operations.record_source_call(
                            SourceCallCoordinate {
                                state: plan.state,
                                statement_index: coordinate.statement_index as usize,
                                call_ordinal: coordinate.call_ordinal as usize,
                            },
                            *source_site,
                            id,
                            *target_state,
                        )?;
                        operations.push(Operation {
                            id,
                            result: OperationResult::Structural(StructuralOperationResult {
                                place,
                                structural_type,
                                multiplicity: match result.multiplicity {
                                    Multiplicity::Affine => StructuralMultiplicity::Affine,
                                    Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                                    Multiplicity::Linear => return unsupported("ordinary structural result has unsupported linear custody"),
                                },
                                qualifications: Vec::new(),
                                projected_qualifications: Vec::new(),
                                claims: Vec::new(),
                            }),
                            kind: OperationKind::CallStructuralWithScalarArguments {
                                callee: lookup_machine_id(&machine_ids, *target_machine)?,
                                arguments: terminal_scalar_arguments,
                                structural_arguments: terminal_arguments,
                                claim_transfers: Vec::new(),
                                returned_claim_transfers: Vec::new(),
                                requirement_obligations,
                                crash_continuations,
                            },
                        });
                        structural_result_places.push((
                            StructuralPlaceDeclaration {
                                id: place,
                                kind: StructuralPlaceKind::OperationResult {
                                    producer: id,
                                    structural_type,
                                },
                            },
                            *discard_result_on_return,
                        ));
                        scalar_arrays::bind_local(
                            checked,
                            plan,
                            operation,
                            place,
                            &mut evaluation.array_locals,
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
                            &affine_scalar_record_places,
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
                        )?;
                        OperationKind::CallStructuralScalar {
                            callee,
                            arguments,
                            structural_arguments: lower_structural_arguments(
                                structural_arguments,
                                parameters,
                                &local_places,
                                &affine_scalar_record_places,
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
                    } else {
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
                        &[],
                        &target.structural_parameters,
                        &type_ids,
                        &structural_types,
                        &[],
                        &primitive_local_places,
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
                        &[],
                        std::slice::from_ref(&target.structural_parameter),
                        &type_ids,
                        &structural_types,
                        &[],
                        &primitive_local_places,
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
                        &[],
                        &structural_result_places,
                        &target.structural_parameters,
                        &type_ids,
                        &structural_types,
                        &expected_claim_arguments,
                        &primitive_local_places,
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
                        &[],
                        &structural_result_places,
                        &target.structural_parameters,
                        &type_ids,
                        &structural_types,
                        &expected_claim_arguments,
                        &primitive_local_places,
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
                    } else {
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
                        &[],
                        &structural_result_places,
                        &target.structural_parameters,
                        &type_ids,
                        &structural_types,
                        &expected_claim_arguments,
                        &primitive_local_places,
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
                    ..
                } => {
                    let (destination, destination_type) = match destination {
                        checked_trees::CheckedPrimitiveStoreDestination::Parameter {
                            parameter_index,
                        } => {
                            let parameter = parameters.get(*parameter_index as usize).ok_or(
                                LoweringError::Unsupported("primitive store parameter is absent"),
                            )?;
                            if !matches!(
                                parameter.access,
                                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                            ) || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                                || !parameter.qualifications.is_empty()
                            {
                                return unsupported(
                                    "primitive store parameter lost its exclusive custody",
                                );
                            }
                            let declaration = structural_types
                                .iter()
                                .find(|declaration| declaration.id == parameter.structural_type)
                                .ok_or(LoweringError::Unsupported(
                                    "primitive store type is absent",
                                ))?;
                            let StructuralTypeShape::PrimitiveScalar(scalar_type) =
                                declaration.shape
                            else {
                                return unsupported("primitive store destination is not primitive");
                            };
                            (parameter.place, scalar_type)
                        }
                        checked_trees::CheckedPrimitiveStoreDestination::Local { symbol } => {
                            let local = primitive_locals::find(&primitive_local_places, *symbol)?;
                            (local.declaration.id, local.scalar_type)
                        }
                    };
                    let value = crate::scalar_bindings::ScalarBindings::new(source_value_count)
                        .with_primitive_storage(&evaluation.primitive_storage)
                        .expression_at(
                            checked,
                            plan.state,
                            *statement_index,
                            CheckedScalarExpressionRole::AssignmentValue,
                        )?;
                    crate::primitive_store::emit_value(
                        destination,
                        destination_type,
                        &value,
                        &scalar_result_values,
                        &mut next_value_identity,
                        &mut operations,
                    )?
                }
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) => {
                    crate::structural_byte_sequence_store::emit(
                        store,
                        parameters,
                        &mut structural_types,
                        &mut literal_places,
                        &mut next_place,
                        &mut next_value_identity,
                        &mut next_call_obligation,
                        &mut operations,
                    )?
                }
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
                    let bindings =
                        crate::scalar_bindings::ScalarBindings::new(scalar_result_values.len())
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
                    crate::structural_byte_sequence_index_store::emit(
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
                    let bindings =
                        crate::scalar_bindings::ScalarBindings::new(scalar_result_values.len())
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
                    crate::byte_sequence_write::emit(
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
                            parameter.position == store.destination_parameter_position
                        })
                        .ok_or(LoweringError::Unsupported(
                            "structural scalar store names an unknown parameter",
                        ))?;
                    let lowered =
                        crate::structural_scalar_store::lower_structural_scalar_store_place(
                            store,
                            store.statement_index,
                            destination,
                            &structural_types,
                            crate::structural_scalar_store::StoreAccessPolicy::Exclusive,
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
                    OperationKind::StructuralScalarFieldStore {
                        destination: destination.place,
                        path: lowered.path,
                        field: lowered.field,
                        value: value.id,
                    }
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
        let trivial_affine_discards = structural_result_places
            .iter()
            .rev()
            .filter_map(|(place, discard)| discard.then_some(Ok(place.id)))
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
                let place = place_id(allocate_dense(&mut next_place)?);
                let structural_type = lookup_type_id(&type_ids, &result.type_identity)?;
                Ok::<_, LoweringError>((
                    source,
                    terminal_psi::StructuralResultDeclaration {
                        place,
                        structural_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
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
                        &predicate_parameters
                            .iter()
                            .find(|(symbol, _)| *symbol == plan.machine)
                            .expect("every closure machine has predicate parameter bindings")
                            .1,
                        &structural_types,
                        runtime_requirements,
                    )?
                }
            } else {
                Vec::new()
            };
        evaluation.blocks.push(Block {
            structural_parameters: Vec::new(),
            id: block,
            parameters: evaluation.parameters,
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
            .flat_map(|block| crate::scalar_computations::arrays::declarations(&block.operations))
            .filter(|place| {
                !structural_result_places
                    .iter()
                    .any(|(existing, _)| existing.id == place.id)
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
            .chain(affine_scalar_record_places.iter().copied())
            .chain(primitive_local_places.iter().map(|local| local.declaration))
            .chain(literal_places.iter().copied())
            .chain(subslice_places.iter().copied())
            .chain(structural_result_places.iter().map(|(place, _)| *place))
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
        machines.push(TerminalMachine {
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
                ensures: Vec::new(),
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
            let mut lowered = crate::structural_scalar_return::lower_structural_scalar_return_machine_in_namespace(
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
            let mut emitted = crate::boundary_scalar_return::emit_boundary_scalar_return(
                checked,
                plan,
                lower_unit_parameters(
                    &plan.structural_parameters,
                    &type_ids,
                    &domain_ids,
                    &mut next_place,
                )?,
                crate::boundary_scalar_return::BoundaryScalarReturnCatalogs {
                    structural_types: &structural_types,
                    type_ids: &type_ids,
                    service_ids: &service_ids,
                },
                crate::boundary_scalar_return::BoundaryScalarReturnIdentities {
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
        let mut lowered = crate::scalar_graph_module::build_scalar_graph_module_in_namespace(
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
        let [terminal_machine] = lowered.semantic_module.machines.as_slice() else {
            unreachable!("one prepared selected scalar graph emits one terminal machine")
        };
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
        crate::affine_return::lower_claim_free_affine_return_machines(
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
            scalar_range_invariants: Vec::new(),
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

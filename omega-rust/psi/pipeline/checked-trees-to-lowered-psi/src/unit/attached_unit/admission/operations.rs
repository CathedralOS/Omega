//! Validate each operation's exact source, callee, transfer and boundary custody.

use super::super::super::{
    CheckedBoundaryMachineResultPlan, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, LoweringError, terminal_scalar_type, unsupported,
};
use super::super::selected_operator::{
    validate_selected_operator_scalar_call, validate_selected_operator_structural_call,
    validate_selected_operator_structural_scalar_call,
};
use super::super::{
    UnitBody, checked_unit_target_reach_matches, primitive_locals, provider_attachments,
    reference_results, retain_exact_checked_flow_call, retain_exact_flow_call,
    retain_exact_unit_boundary, scalar_arrays, scalar_structural_calls, structural_calls,
    unique_unit_boundary,
};
use crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee;
use checked_trees::CheckedBoundaryMachinePlan;
use symbols::SymbolHandle;

pub(super) fn validate<'a>(
    checked: &'a CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
    closure: &[SymbolHandle],
    scalar_closure: &[SymbolHandle],
    boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, String)>,
) -> Result<(), LoweringError> {
    let plans = &checked.facts.flow.terminal_unit_effects;
    for (operation_index, operation) in machine.operations.iter().enumerate() {
        provider_attachments::validate_call_source(
            checked,
            machine.machine,
            machine.state,
            operation,
            &machine.provider_attachment_requirements,
        )?;
        crate::emission::call_source_custody::validate_operation(
            checked,
            machine.machine,
            machine.state,
            operation,
            &machine.structural_parameters,
        )?;
        structural_calls::validate_custody(checked, machine.machine, machine.state, operation)?;
        match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. } => {
                crate::expression_preparation::source_custody::structural::validate(
                    checked,
                    machine.machine,
                    machine.state,
                    operation,
                )?;
            }
            CheckedUnitEffectOperationPlan::EstablishReference { .. } => {
                reference_results::validate_establishment(checked, machine, operation)?;
            }
            CheckedUnitEffectOperationPlan::ReleaseReference { .. } => {}
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
                crate::emission::call_source_custody::projected_receivers::validate(
                    checked,
                    machine.machine,
                    machine.state,
                    &machine.operations,
                    &machine.structural_parameters,
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
                erased_scalar_arguments: _,
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
                coordinate,
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                service_reach,
                ..
            } => {
                retain_exact_checked_flow_call(checked, machine, *coordinate, *target_state)?;
                retain_exact_unit_boundary(
                    checked,
                    plans,
                    boundaries,
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
                    boundaries,
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
                    boundaries,
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

    Ok(())
}

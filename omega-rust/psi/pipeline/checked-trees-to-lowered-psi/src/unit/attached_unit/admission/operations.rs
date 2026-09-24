//! Validate each operation of an ordinary body: its calls through the shared
//! call admission (`calls`), then the custody of every other operation.

use super::super::super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, LoweringError,
    unsupported,
};
use super::super::bodies::UnitPlans;
use super::super::selected_operator::{
    validate_selected_operator_scalar_call, validate_selected_operator_structural_call,
    validate_selected_operator_structural_scalar_call,
};
use super::super::{provider_attachments, reference_results, scalar_arrays, structural_calls};
use crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee;
use checked_trees::CheckedBoundaryMachinePlan;
use symbols::SymbolHandle;

pub(super) fn validate<'a>(
    checked: &'a CheckedTrees,
    plans: UnitPlans<'a>,
    machine: &CheckedUnitEffectMachinePlan,
    closure: &[SymbolHandle],
    scalar_closure: &[SymbolHandle],
    boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, String)>,
) -> Result<(), LoweringError> {
    let caller = super::CallerView::ordinary(machine);
    for (operation_index, operation) in machine.operations.iter().enumerate() {
        provider_attachments::validate_call_source(
            checked,
            machine.machine,
            machine.state,
            operation,
            &machine.provider_attachment_requirements,
        )?;
        super::calls::admit(checked, plans, &caller, operation, boundaries)?;
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
            CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. } => {
                super::super::view_ranges::binding_local(checked, machine.state, operation)?;
            }
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
            CheckedUnitEffectOperationPlan::ScalarCall { target_machine, .. } => {
                // A scalar callee joins an ordinary body's closure before
                // admission; a composed catalog selects it from the call.
                if !scalar_closure.contains(target_machine)
                    && !(closure.contains(target_machine)
                        && matches!(
                            CheckedScalarCallee::find_for_unit_call(checked, *target_machine)?,
                            CheckedScalarCallee::Operations(_)
                        ))
                {
                    return unsupported(
                        "ordinary Unit scalar call target is absent from the checked closure",
                    );
                }
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
                    scalar_arguments,
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
            // Every other call was admitted by `calls::admit` above.
            CheckedUnitEffectOperationPlan::CallUnit { .. }
            | CheckedUnitEffectOperationPlan::StructuralCall { .. }
            | CheckedUnitEffectOperationPlan::BoundaryCall { .. }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
            | CheckedUnitEffectOperationPlan::PortWrite { .. }
            | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
            | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
            | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
            | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd { .. }
            | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
            | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
            | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
            | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
            | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
            | CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_)
            | CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
            | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
            | CheckedUnitEffectOperationPlan::Complete { .. } => {}
        }
    }

    Ok(())
}

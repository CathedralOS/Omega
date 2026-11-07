//! Independently reconstruct the ordinary calls within a guarded installation.
//! Scope admission chooses the exact bodies; this replay checks the ABI and
//! borrows against the caller's real storage at that operation.

use crate::LegalizationError;
use abstract_operations::{AbstractBoundaryResult, AbstractOperation, AbstractOperationPlan};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use target_operations::{InstallationCallRole, TargetOperationPlan, TargetUnitOperation};

pub(in crate::legalization) fn validate(
    target: &TargetUnitOperation,
    source: &AbstractOperation,
    caller: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    custody: &super::reference_custody::Custody,
) -> Result<(), LegalizationError> {
    let TargetUnitOperation::InstallStaticInterrupt {
        psi_operation,
        boundary,
        arguments,
        calls,
        result_home,
    } = target
    else {
        return Err(LegalizationError::custody());
    };
    let fail = |reason| LegalizationError::InstalledInterruptAssociation {
        operation: Some(*psi_operation),
        reason,
    };
    let AbstractOperation::BoundaryCall {
        psi_operation: source_operation,
        boundary: source_boundary,
        arguments: scalar_arguments,
        structural_arguments,
        result: AbstractBoundaryResult::Scalar(result),
        ..
    } = source
    else {
        return Err(fail("installation changed its source operation or result"));
    };
    let declaration = plan
        .boundary_machines
        .iter()
        .find(|row| row.id == *boundary)
        .ok_or_else(|| fail("installation lost its retained boundary declaration"))?;
    if psi_operation != source_operation
        || boundary != source_boundary
        || !scalar_arguments.is_empty()
        || structural_arguments.as_slice() != arguments
        || unit.boundary_machine(*boundary) != Some(declaration)
        || declaration
            .declaration
            .as_ref()
            .and_then(|row| row.execution.static_interrupt_operation())
            != Some(language_core::static_interrupts::StaticInterruptOperation::Install)
        || result.scalar_type != semantic_vocabulary::ScalarType::Boolean
        || result_home.defining_operation != *psi_operation
        || result_home.source_value != result.value
        || result_home.scalar_type != result.scalar_type
        || Some(result_home.shape) != super::scalar_shape(result.scalar_type)
    {
        return Err(fail(
            "installation changed its exact operands or Boolean refusal result",
        ));
    }
    for (position, call) in calls.iter().enumerate() {
        let (role, source_position) = if position == 0 {
            (InstallationCallRole::Validate, 0)
        } else {
            (InstallationCallRole::Work, 5)
        };
        if call.role != role {
            return Err(fail(
                "installation changed the order of validation and retained work",
            ));
        }
        let expected = super::callee_plan(call.callee, native, plan, unit)?;
        let signature = super::call_signature::signature(call.callee, plan)?;
        if signature.scalar_parameter_count() != 0
            || signature.structural_parameter_count() != 1
            || call.call_plan != expected
        {
            return Err(fail(
                "installation helper changed its ordinary calling plan",
            ));
        }
        let mut actual = arguments[source_position].clone();
        actual.access = terminal_psi::StructuralAccess::SharedBorrow;
        let expected_argument = super::structural_call::argument_at(
            &actual,
            0,
            *psi_operation,
            caller,
            signature,
            &expected,
            native,
            plan,
            custody,
        )?;
        if call.argument != expected_argument {
            return Err(fail(
                "installation helper changed its original table or context borrow",
            ));
        }
    }
    Ok(())
}

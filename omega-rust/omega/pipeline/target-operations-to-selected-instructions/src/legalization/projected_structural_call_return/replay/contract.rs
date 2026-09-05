//! Independent full-carrier contract for the proposed legalization record.

use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use legalized_operations::{
    LegalizedProjectedStructuralCallReturn, ProjectedStructuralCallReturnLegalizationRecipe,
};
use optimization_unit::PsiOptimizationUnit;
use target_operations::{TargetOperation, TargetOperationPlan};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

use crate::LegalizationError;
use crate::legalization::model::{
    ProjectedStructuralCallReturnLegalizationError as FamilyError,
    ProjectedStructuralCallReturnLegalizationReceipt,
};

pub(super) fn validate(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    closure: &LegalizedProjectedStructuralCallReturn,
) -> Result<ProjectedStructuralCallReturnLegalizationReceipt, LegalizationError> {
    let (
        [target_caller, target_callee],
        [source_caller, source_callee],
        [unit_caller, unit_callee],
    ) = (
        target.functions.as_slice(),
        abstract_plan.functions.as_slice(),
        unit.functions.as_slice(),
    )
    else {
        return Err(FamilyError::UnsupportedSourceShape.into());
    };
    let TargetOperation::ReturnStructuralCall {
        psi_edge,
        psi_operation,
        operation_result,
        result,
        callee,
        structural_types,
        callee_call_plan,
        structural_parameters,
        arguments,
        claim_transfers,
        returned_claim_transfers,
        returned_claims,
        requirement_obligations,
        crash_continuations,
        ..
    } = &target_caller.operation
    else {
        return Err(FamilyError::UnsupportedTargetShape.into());
    };
    let TargetOperation::ReturnStructuralParameter {
        call_plan,
        parameters,
        source,
        result: callee_result,
        psi_edge: callee_edge,
        returned_claims: callee_returned_claims,
        trivial_affine_locals,
        trivial_affine_discards,
        ..
    } = &target_callee.operation
    else {
        return Err(FamilyError::UnsupportedTargetShape.into());
    };
    let ([caller_parameter], Some(caller_result)) = (
        source_caller.structural_parameters.as_slice(),
        source_caller.result.structural(),
    ) else {
        return Err(FamilyError::UnsupportedSourceShape.into());
    };
    let [
        AbstractOperation::CallStructural {
            psi_operation: source_operation,
            result: source_operation_result,
            callee: source_callee_id,
            structural_arguments,
            claim_transfers: source_claim_transfers,
            returned_claim_transfers: source_returned_transfers,
            requirement_obligations: source_requirements,
            crash_continuations: source_crashes,
            ..
        },
        AbstractOperation::ReturnStructural {
            psi_edge: source_edge,
            source: returned_source,
            returned_claims: source_returned_claims,
            trivial_affine_locals: caller_locals,
            trivial_affine_discards: caller_discards,
        },
    ] = source_caller.operations.as_slice()
    else {
        return Err(FamilyError::UnsupportedSourceShape.into());
    };
    let ([callee_parameter], Some(source_callee_result)) = (
        source_callee.structural_parameters.as_slice(),
        source_callee.result.structural(),
    ) else {
        return Err(FamilyError::UnsupportedSourceShape.into());
    };
    let [
        AbstractOperation::ReturnStructural {
            psi_edge: source_callee_edge,
            source: callee_source,
            returned_claims: source_callee_claims,
            trivial_affine_locals: source_callee_locals,
            trivial_affine_discards: source_callee_discards,
        },
    ] = source_callee.operations.as_slice()
    else {
        return Err(FamilyError::UnsupportedSourceShape.into());
    };
    let ([target_parameter], [target_argument]) =
        (structural_parameters.as_slice(), arguments.as_slice())
    else {
        return Err(FamilyError::UnsupportedTargetShape.into());
    };
    let rows = &caller_parameter.projected_qualifications;
    if closure.recipe != ProjectedStructuralCallReturnLegalizationRecipe::OwnedLinearDirectV1
        || closure.caller != *target_caller
        || closure.callee != *target_callee
        || closure.caller_entry_block != unit_caller.entry
        || closure.callee_entry_block != unit_callee.entry
        || !super::custody::nodes_match(&closure.caller_nodes, unit_caller)
        || !super::custody::nodes_match(&closure.callee_nodes, unit_callee)
        || target.entry != target_caller.machine
        || abstract_plan.entry != source_caller.machine
        || target_caller.machine != source_caller.machine
        || target_callee.machine != source_callee.machine
        || *callee != target_callee.machine
        || *source_callee_id != source_callee.machine
        || target_caller.attachment.is_some()
        || target_callee.attachment.is_some()
        || target_caller.fixed_integer_scalar_abi.is_some()
        || target_callee.fixed_integer_scalar_abi.is_some()
        || structural_types != &abstract_plan.structural_types
        || callee_call_plan != call_plan
        || *psi_operation != *source_operation
        || *psi_edge != *source_edge
        || operation_result != source_operation_result
        || result != caller_result
        || returned_source != &source_operation_result.place
        || claim_transfers != source_claim_transfers
        || returned_claim_transfers != source_returned_transfers
        || returned_claims != source_returned_claims
        || requirement_obligations != source_requirements
        || crash_continuations != source_crashes
        || !caller_locals.is_empty()
        || !caller_discards.is_empty()
        || parameters.as_slice() != std::slice::from_ref(callee_parameter)
        || source != callee_parameter
        || callee_result != source_callee_result
        || *callee_edge != *source_callee_edge
        || callee_returned_claims != source_callee_claims
        || callee_source != &callee_parameter.place
        || !trivial_affine_locals.is_empty()
        || !trivial_affine_discards.is_empty()
        || !source_callee_locals.is_empty()
        || !source_callee_discards.is_empty()
        || structural_arguments.len() != 1
        || structural_arguments[0].place != caller_parameter.place
        || structural_arguments[0].access != StructuralAccess::Owned
        || !structural_arguments[0].path.is_empty()
        || target_parameter.place != caller_parameter.place
        || target_parameter.structural_type != caller_parameter.structural_type
        || target_parameter.multiplicity != StructuralMultiplicity::Linear
        || target_parameter.access != StructuralAccess::Owned
        || target_parameter.projected_qualifications != *rows
        || target_argument.place != caller_parameter.place
        || target_argument.access != StructuralAccess::Owned
        || !target_argument.path.is_empty()
        || caller_parameter.multiplicity != StructuralMultiplicity::Linear
        || caller_parameter.access != StructuralAccess::Owned
        || callee_parameter.multiplicity != StructuralMultiplicity::Linear
        || callee_parameter.access != StructuralAccess::Owned
        || rows.is_empty()
        || rows != &caller_result.projected_qualifications
        || rows != &source_operation_result.projected_qualifications
        || rows != &callee_parameter.projected_qualifications
        || rows != &source_callee_result.projected_qualifications
        || !super::custody::function_matches(unit_caller, source_caller, 2)
        || !super::custody::function_matches(unit_callee, source_callee, 1)
    {
        return Err(FamilyError::NonCanonicalProposedClosure.into());
    }
    Ok(ProjectedStructuralCallReturnLegalizationReceipt {
        caller: target_caller.machine,
        callee: target_callee.machine,
        projected_qualification_count: rows.len(),
    })
}

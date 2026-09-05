//! Producer grammar for the one exact result-bearing structural closure.

use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use optimization_unit::PsiOptimizationFunction;
use target_operations::{
    TargetFunction, TargetOperation, TargetOperationPlan, TargetStructuralParameter,
};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration};

use crate::LegalizationError;
use crate::legalization::model::ProjectedStructuralCallReturnLegalizationError as FamilyError;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_pair(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    target_caller: &TargetFunction,
    target_callee: &TargetFunction,
    source_caller: &abstract_operations::AbstractFunction,
    source_callee: &abstract_operations::AbstractFunction,
    unit_caller: &PsiOptimizationFunction,
    unit_callee: &PsiOptimizationFunction,
) -> Result<(), LegalizationError> {
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
    let ([source_parameter], Some(source_result)) = (
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
    if target.entry != target_caller.machine
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
        || result != source_result
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
        || structural_arguments[0].place != source_parameter.place
        || structural_arguments[0].access != StructuralAccess::Owned
        || !structural_arguments[0].path.is_empty()
        || !target_parameter_matches(target_parameter, source_parameter)
        || target_argument.place != source_parameter.place
        || target_argument.access != StructuralAccess::Owned
        || !target_argument.path.is_empty()
        || source_parameter.multiplicity != StructuralMultiplicity::Linear
        || callee_parameter.multiplicity != StructuralMultiplicity::Linear
        || source_parameter.access != StructuralAccess::Owned
        || callee_parameter.access != StructuralAccess::Owned
        || !same_nonempty_roster(
            source_parameter,
            source_result,
            source_operation_result,
            callee_parameter,
            source_callee_result,
        )
        || !unit_function_matches(unit_caller, source_caller, 2)
        || !unit_function_matches(unit_callee, source_callee, 1)
    {
        return Err(FamilyError::SourceTargetMismatch.into());
    }
    Ok(())
}

fn target_parameter_matches(
    target: &TargetStructuralParameter,
    source: &StructuralParameterDeclaration,
) -> bool {
    target.place == source.place
        && target.structural_type == source.structural_type
        && target.multiplicity == source.multiplicity
        && target.access == source.access
        && target.projected_qualifications == source.projected_qualifications
}

fn same_nonempty_roster(
    parameter: &StructuralParameterDeclaration,
    result: &terminal_psi::StructuralResultDeclaration,
    operation_result: &terminal_psi::StructuralOperationResult,
    callee_parameter: &StructuralParameterDeclaration,
    callee_result: &terminal_psi::StructuralResultDeclaration,
) -> bool {
    let rows = &parameter.projected_qualifications;
    !rows.is_empty()
        && rows == &result.projected_qualifications
        && rows == &operation_result.projected_qualifications
        && rows == &callee_parameter.projected_qualifications
        && rows == &callee_result.projected_qualifications
}

fn unit_function_matches(
    unit: &PsiOptimizationFunction,
    source: &abstract_operations::AbstractFunction,
    node_count: usize,
) -> bool {
    let [block] = unit.blocks.as_slice() else {
        return false;
    };
    unit.machine == source.machine
        && unit.entry == source.entry
        && unit.attachment == source.attachment
        && unit.parameters.is_empty()
        && unit.structural_parameters == source.structural_parameters
        && unit.result == source.result
        && unit.entry_claim_declarations == source.entry_claims
        && unit.published_service_ceiling == source.published_service_ceiling
        && block.id == source.entry
        && block.parameters.is_empty()
        && block.nodes.len() == node_count
        && block
            .nodes
            .iter()
            .map(|node| &node.operation)
            .eq(source.operations.iter())
}

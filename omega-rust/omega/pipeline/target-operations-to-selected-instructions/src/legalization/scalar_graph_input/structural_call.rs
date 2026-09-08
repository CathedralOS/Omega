//! Source and target joins for whole borrowed arguments in scalar helper calls.
use crate::LegalizationError;
use abstract_operations::{AbstractFunction, AbstractOperation, AbstractOperationPlan};
use calling_conventions::{CallPlan, ValueShape};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::MachineId;
use target_operations::{
    TargetFunction, TargetOperation, TargetOperationPlan, TargetStructuralArgument,
};
use terminal_psi::{StructuralAccess, StructuralArgument};

pub(in crate::legalization) fn validate_argument(
    argument: &StructuralArgument,
    target_argument: &TargetStructuralArgument,
    caller: &PsiOptimizationFunction,
    callee: MachineId,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<CallPlan, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let target_caller = native
        .functions
        .iter()
        .find(|function| function.machine == caller.machine)
        .ok_or(invalid.clone())?;
    let parameters = super::structural_parameters(target_caller).ok_or(invalid.clone())?;
    let [source] = caller.structural_parameters.as_slice() else {
        return Err(invalid);
    };
    let [parameter] = parameters else {
        return Err(invalid);
    };
    let callee_plan = super::callee_plan(callee, native, plan, unit)?;
    let callee = unit
        .functions
        .iter()
        .find(|function| function.machine == callee)
        .ok_or(invalid.clone())?;
    let [callee_parameter] = callee.structural_parameters.as_slice() else {
        return Err(invalid);
    };
    let destination = callee_plan
        .parameters
        .get(callee.parameters.len())
        .ok_or(invalid.clone())?;
    if argument.place != source.place
        || argument.access != StructuralAccess::SharedBorrow
        || !argument.path.is_empty()
        || source.access != argument.access
        || callee_parameter.access != argument.access
        || source.structural_type != callee_parameter.structural_type
        || source.multiplicity != callee_parameter.multiplicity
        || parameter.place != source.place
        || parameter.access != source.access
        || parameter.structural_type != source.structural_type
        || target_argument.place != source.place
        || target_argument.access != argument.access
        || !target_argument.path.is_empty()
        || target_argument.root_structural_type != source.structural_type
        || target_argument.structural_type != source.structural_type
        || target_argument.shape != ValueShape::borrowed_reference(16, 8)
        || parameter.shape != target_argument.shape
        || target_argument.source != parameter.placement
        || target_argument.destination != *destination
        || target_argument.source_byte_offset != 0
        || target_argument.fixed_array_length.is_some()
        || target_argument.element_stride.is_some()
    {
        return Err(invalid);
    }
    Ok(callee_plan)
}

/// Reconstruct the whole-reference transport from validated caller and callee ABIs.
/// Ordered calls whose results are unused have no target expression tree.
pub(in crate::legalization) fn argument(
    semantic: &StructuralArgument,
    caller: &PsiOptimizationFunction,
    callee: MachineId,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<TargetStructuralArgument, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let parameter = native
        .functions
        .iter()
        .find(|function| function.machine == caller.machine)
        .and_then(super::structural_parameters)
        .and_then(|parameters| {
            parameters
                .iter()
                .find(|parameter| parameter.place == semantic.place)
        })
        .ok_or(invalid.clone())?;
    let call = super::callee_plan(callee, native, plan, unit)?;
    let destination = call.parameters.last().ok_or(invalid.clone())?;
    let argument = TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: semantic.path.clone(),
        root_structural_type: parameter.structural_type,
        structural_type: parameter.structural_type,
        shape: parameter.shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source: parameter.placement.clone(),
        destination: destination.clone(),
    };
    validate_argument(semantic, &argument, caller, callee, native, plan, unit)?;
    Ok(argument)
}

pub(super) fn validate_target(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let TargetOperation::ReturnStructuralScalarCall {
        psi_edge,
        psi_operation,
        source_value,
        scalar_type,
        callee,
        structural_types,
        call_plan,
        structural_parameters,
        arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = &target.operation
    else {
        return Err(invalid);
    };
    let [
        AbstractOperation::CallStructuralScalar {
            psi_operation: operation,
            result,
            callee: source_callee,
            arguments: scalars,
            structural_arguments,
            claim_transfers: claims,
            requirement_obligations: requirements,
            crash_continuations: crashes,
        },
        AbstractOperation::Return {
            psi_edge: edge,
            result: returned_result,
            value,
            scalar_type: returned_type,
            cleanup_actions,
        },
    ] = abstracted.operations.as_slice()
    else {
        return Err(invalid);
    };
    let ([argument], [target_argument]) = (structural_arguments.as_slice(), arguments.as_slice())
    else {
        return Err(invalid);
    };
    let abi = target
        .mixed_structural_scalar_abi
        .as_ref()
        .ok_or(invalid.clone())?;
    if psi_edge != edge
        || psi_operation != operation
        || source_value != value
        || result.value != *value
        || result.scalar_type != *scalar_type
        || returned_type != scalar_type
        || abstracted.result.scalar().map(|result| result.value) != Some(*returned_result)
        || callee != source_callee
        || structural_types != &plan.structural_types
        || call_plan != &abi.call_plan
        || structural_parameters != &abi.structural_parameters
        || !scalars.is_empty()
        || !cleanup_actions.is_empty()
        || claim_transfers != claims
        || !claims.is_empty()
        || requirement_obligations != requirements
        || !requirements.is_empty()
        || crash_continuations != crashes
        || !crashes.is_empty()
    {
        return Err(invalid);
    }
    validate_argument(
        argument,
        target_argument,
        optimized,
        *callee,
        native,
        plan,
        unit,
    )?;
    Ok(())
}

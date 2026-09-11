//! Bounded whole-root structural-call/scalar-return target selection.
mod ordinary;
pub(super) use ordinary::lower as lower_ordinary;

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::{AbstractFunction, AbstractOperation, AbstractResult};
use calling_conventions::{CallPlan, ValueShape};
use semantic_vocabulary::{MachineId, StructuralTypeId};
use target::NativeTarget;
use target_operations::{
    TargetFunction, TargetOperation, TargetStructuralArgument, TargetStructuralParameter,
    TerminalPsiProvenance,
};

use super::{LoweringError, scalar_shape};
use crate::lowering::shared::StructuralTypeLookup;
use crate::lowering::structural_signature::StructuralCallSignature;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_direct_return(
    function: &AbstractFunction,
    function_result: AbstractResult,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &StructuralTypeLookup<'_>,
    call_plan: &CallPlan,
    target_structural_parameters: &[TargetStructuralParameter],
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<Option<TargetFunction>, LoweringError> {
    let [
        AbstractOperation::CallStructuralScalar {
            psi_operation,
            result: call_result,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        },
        AbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
    else {
        return Ok(None);
    };
    if *result != function_result.value
        || *value != call_result.value
        || *scalar_type != function_result.scalar_type
        || call_result.scalar_type != function_result.scalar_type
        || !cleanup_actions.is_empty()
        || !arguments.is_empty()
        || structural_arguments.is_empty()
        || structural_arguments
            .iter()
            .any(|argument| !argument.path.is_empty())
    {
        return Ok(None);
    }

    let callee_function = functions
        .get(callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(*callee))?;
    let callee_result = callee_function.result.scalar().ok_or(
        LoweringError::UnsupportedOperationInScalarFunction(function.machine),
    )?;
    if !callee_function.parameters.is_empty()
        || callee_result.scalar_type != call_result.scalar_type
        || structural_arguments.len() != callee_function.structural_parameters.len()
    {
        return Err(LoweringError::UnsupportedOperationInScalarFunction(
            function.machine,
        ));
    }
    let signature = StructuralCallSignature::derive(
        &[],
        &callee_function.structural_parameters,
        Some(scalar_shape(
            callee_result.value,
            callee_result.scalar_type,
            false,
        )?),
        structural_types,
        shape_cache,
        active,
    )?;
    let callee_shapes = signature.structural_shapes();
    let callee_plan = signature.plan(target)?;
    let parameters_by_place = target_structural_parameters
        .iter()
        .map(|parameter| (parameter.place, parameter))
        .collect::<BTreeMap<_, _>>();
    let arguments = structural_arguments
        .iter()
        .zip(&callee_function.structural_parameters)
        .zip(callee_shapes.iter().copied())
        .zip(&callee_plan.parameters)
        .map(|(((argument, callee_parameter), shape), destination)| {
            let source = parameters_by_place.get(&argument.place).copied().ok_or(
                LoweringError::UnknownStructuralArgumentPlace {
                    machine: function.machine,
                    place: argument.place,
                },
            )?;
            if source.structural_type != callee_parameter.structural_type
                || source.shape != shape
                || argument.access != callee_parameter.access
            {
                return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                    callee: *callee,
                    place: argument.place,
                });
            }
            Ok(TargetStructuralArgument {
                place: argument.place,
                access: argument.access,
                path: Vec::new(),
                root_structural_type: source.structural_type,
                structural_type: source.structural_type,
                shape,
                source_byte_offset: 0,
                fixed_array_length: None,
                element_stride: None,
                source: source.placement.clone().into(),
                destination: destination.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance: TerminalPsiProvenance {
            operations: vec![*psi_operation],
            edges: vec![*psi_edge],
        },
        operation: TargetOperation::ReturnStructuralScalarCall {
            psi_edge: *psi_edge,
            psi_operation: *psi_operation,
            source_value: call_result.value,
            scalar_type: call_result.scalar_type,
            callee: *callee,
            structural_types: structural_types.catalog().clone(),
            call_plan: call_plan.clone(),
            structural_parameters: target_structural_parameters.to_vec(),
            arguments,
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        },
    }))
}

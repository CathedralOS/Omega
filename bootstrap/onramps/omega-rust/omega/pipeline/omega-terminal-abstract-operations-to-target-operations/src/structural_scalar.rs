//! Bounded whole-root structural-call/scalar-return target selection.

use std::collections::{BTreeMap, BTreeSet};

use omega_calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, ValueShape, evaluate_call_plan,
};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::{
    TerminalAbstractFunction, TerminalAbstractOperation, TerminalAbstractResult,
};
use omega_terminal_target_operations::{
    TerminalPsiProvenance, TerminalTargetFunction, TerminalTargetOperation,
    TerminalTargetStructuralArgument, TerminalTargetStructuralParameter,
};
use psi_core::{MachineId, StructuralTypeId};
use psi_terminal::StructuralTypeDeclaration;

use super::{LoweringError, scalar_shape, structural_shape};

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_direct_return(
    function: &TerminalAbstractFunction,
    function_result: TerminalAbstractResult,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    call_plan: &CallPlan,
    target_structural_parameters: &[TerminalTargetStructuralParameter],
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<Option<TerminalTargetFunction>, LoweringError> {
    let [
        TerminalAbstractOperation::CallStructuralScalar {
            psi_operation,
            result: call_result,
            callee,
            structural_arguments,
            claim_transfers,
        },
        TerminalAbstractOperation::Return {
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
    let callee_shapes = callee_function
        .structural_parameters
        .iter()
        .map(|parameter| {
            structural_shape(
                parameter.structural_type,
                structural_types,
                shape_cache,
                active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let callee_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: callee_shapes.clone(),
            result: Some(scalar_shape(
                callee_result.value,
                callee_result.scalar_type,
                false,
            )?),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let parameters_by_place = target_structural_parameters
        .iter()
        .map(|parameter| (parameter.place, parameter))
        .collect::<BTreeMap<_, _>>();
    let arguments = structural_arguments
        .iter()
        .zip(&callee_function.structural_parameters)
        .zip(callee_shapes)
        .zip(&callee_plan.parameters)
        .map(|(((argument, callee_parameter), shape), destination)| {
            let source = parameters_by_place.get(&argument.place).copied().ok_or(
                LoweringError::UnknownStructuralArgumentPlace {
                    machine: function.machine,
                    place: argument.place,
                },
            )?;
            if source.structural_type != callee_parameter.structural_type || source.shape != shape {
                return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                    callee: *callee,
                    place: argument.place,
                });
            }
            Ok(TerminalTargetStructuralArgument {
                place: argument.place,
                path: Vec::new(),
                root_structural_type: source.structural_type,
                structural_type: source.structural_type,
                shape,
                source_byte_offset: 0,
                fixed_array_length: None,
                element_stride: None,
                source: source.placement.clone(),
                destination: destination.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(TerminalTargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: TerminalPsiProvenance {
            operations: vec![*psi_operation],
            edges: vec![*psi_edge],
        },
        operation: TerminalTargetOperation::ReturnStructuralScalarCall {
            psi_edge: *psi_edge,
            psi_operation: *psi_operation,
            source_value: call_result.value,
            scalar_type: call_result.scalar_type,
            callee: *callee,
            structural_types: structural_types
                .values()
                .map(|declaration| (*declaration).clone())
                .collect(),
            call_plan: call_plan.clone(),
            structural_parameters: target_structural_parameters.to_vec(),
            arguments,
            claim_transfers: claim_transfers.clone(),
        },
    }))
}

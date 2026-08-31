//! Bounded whole-root structural-call/structural-return target selection.

use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use omega_calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, ValueClass, ValueShape, evaluate_call_plan,
};
use omega_target::NativeTarget;
use omega_target_operations::{
    TargetFunction, TargetOperation, TargetStructuralArgument, TargetStructuralParameter,
    TerminalPsiProvenance,
};
use psi_core::{MachineId, StructuralTypeId};
use psi_terminal::{StructuralAccess, StructuralMultiplicity, StructuralTypeDeclaration};

use super::require_direct_structural_fragments;
use crate::LoweringError;
use crate::lowering::structural_layout::structural_shape;

pub(in crate::lowering) fn lower_direct_return(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<Option<TargetFunction>, LoweringError> {
    let Some(function_result) = function.result.structural() else {
        return Ok(None);
    };
    let [
        AbstractOperation::CallStructural {
            psi_operation,
            result: operation_result,
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        },
        AbstractOperation::ReturnStructural {
            psi_edge,
            source,
            returned_claims,
            trivial_affine_locals,
            trivial_affine_discards,
        },
    ] = function.operations.as_slice()
    else {
        return Ok(None);
    };
    let [parameter] = function.structural_parameters.as_slice() else {
        return Ok(None);
    };
    let [entry_claim] = function.entry_claims.as_slice() else {
        return Ok(None);
    };
    let [argument] = structural_arguments.as_slice() else {
        return Ok(None);
    };
    let [claim_transfer] = claim_transfers.as_slice() else {
        return Ok(None);
    };
    let [returned_transfer] = returned_claim_transfers.as_slice() else {
        return Ok(None);
    };
    let [result_claim] = operation_result.claims.as_slice() else {
        return Ok(None);
    };
    if !function.parameters.is_empty()
        || !function.published_service_ceiling.is_empty()
        || parameter.position != 0
        || parameter.is_self
        || parameter.multiplicity != StructuralMultiplicity::Linear
        || argument.place != parameter.place
        || argument.access != StructuralAccess::Owned
        || !argument.path.is_empty()
        || claim_transfer.argument_index != 0
        || claim_transfer.claim != entry_claim.claim
        || entry_claim.input != parameter.place
        || !entry_claim.path.is_empty()
        || operation_result.place != *source
        || operation_result.structural_type != function_result.structural_type
        || operation_result.multiplicity != StructuralMultiplicity::Linear
        || operation_result.qualifications != function_result.qualifications
        || result_claim.claim != entry_claim.claim
        || !result_claim.path.is_empty()
        || returned_transfer.caller_claim != entry_claim.claim
        || returned_claims.as_slice() != [entry_claim.claim]
        || !trivial_affine_locals.is_empty()
        || !trivial_affine_discards.is_empty()
    {
        return Ok(None);
    }

    let callee_function = functions
        .get(callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(*callee))?;
    let Some(callee_result) = callee_function.result.structural() else {
        return Ok(None);
    };
    let [callee_parameter] = callee_function.structural_parameters.as_slice() else {
        return Ok(None);
    };
    let [callee_entry_claim] = callee_function.entry_claims.as_slice() else {
        return Ok(None);
    };
    let [
        AbstractOperation::ReturnStructural {
            source: callee_source,
            returned_claims: callee_returned_claims,
            trivial_affine_locals: callee_locals,
            trivial_affine_discards: callee_discards,
            ..
        },
    ] = callee_function.operations.as_slice()
    else {
        return Ok(None);
    };
    if !callee_function.parameters.is_empty()
        || !callee_function.published_service_ceiling.is_empty()
        || callee_parameter.position != 0
        || callee_parameter.is_self
        || callee_parameter.multiplicity != StructuralMultiplicity::Linear
        || callee_parameter.structural_type != parameter.structural_type
        || callee_parameter.qualifications != parameter.qualifications
        || callee_result.structural_type != function_result.structural_type
        || callee_result.multiplicity != StructuralMultiplicity::Linear
        || callee_result.qualifications != function_result.qualifications
        || callee_entry_claim.input != callee_parameter.place
        || !callee_entry_claim.path.is_empty()
        || *callee_source != callee_parameter.place
        || callee_returned_claims.as_slice() != [callee_entry_claim.claim]
        || returned_transfer.callee_claim != callee_entry_claim.claim
        || !callee_locals.is_empty()
        || !callee_discards.is_empty()
    {
        return Ok(None);
    }

    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let shape = structural_shape(
        parameter.structural_type,
        structural_types,
        &mut cache,
        &mut active,
    )?;
    if shape.class != ValueClass::Integer
        || !((shape.byte_size == 8 && shape.alignment == 8) || (9..=16).contains(&shape.byte_size))
    {
        return Err(LoweringError::UnsupportedStructuralReturnShape {
            machine: function.machine,
            byte_size: shape.byte_size,
        });
    }
    let caller_plan = direct_plan(target, shape)?;
    let callee_plan = direct_plan(target, shape)?;
    let source_placement = caller_plan.parameters.first().cloned().ok_or(
        LoweringError::AbiParameterCountMismatch {
            expected: 1,
            actual: 0,
        },
    )?;
    let destination = callee_plan.parameters.first().cloned().ok_or(
        LoweringError::AbiParameterCountMismatch {
            expected: 1,
            actual: 0,
        },
    )?;
    let caller_result = caller_plan
        .result
        .clone()
        .ok_or(LoweringError::UnsupportedStructuralReturn(function.machine))?;
    let callee_result_placement = callee_plan
        .result
        .clone()
        .ok_or(LoweringError::UnsupportedStructuralReturn(function.machine))?;
    require_direct_structural_fragments(function.machine, &source_placement)?;
    require_direct_structural_fragments(function.machine, &destination)?;
    require_direct_structural_fragments(function.machine, &caller_result)?;
    require_direct_structural_fragments(function.machine, &callee_result_placement)?;
    if caller_result != callee_result_placement {
        return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
    }

    let target_parameter = TargetStructuralParameter {
        place: parameter.place,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        access: StructuralAccess::Owned,
        shape,
        placement: source_placement.clone(),
    };
    Ok(Some(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        fixed_integer_scalar_abi: None,
        provenance: TerminalPsiProvenance {
            operations: vec![*psi_operation],
            edges: vec![*psi_edge],
        },
        operation: TargetOperation::ReturnStructuralCall {
            psi_edge: *psi_edge,
            psi_operation: *psi_operation,
            operation_result: operation_result.clone(),
            result: function_result.clone(),
            callee: *callee,
            structural_types: structural_types
                .values()
                .map(|value| (*value).clone())
                .collect(),
            call_plan: caller_plan,
            callee_call_plan: callee_plan,
            structural_parameters: vec![target_parameter],
            arguments: vec![TargetStructuralArgument {
                place: argument.place,
                access: argument.access,
                path: Vec::new(),
                root_structural_type: parameter.structural_type,
                structural_type: parameter.structural_type,
                shape,
                source_byte_offset: 0,
                fixed_array_length: None,
                element_stride: None,
                source: source_placement.clone(),
                destination: destination.clone(),
            }],
            claim_transfers: claim_transfers.clone(),
            returned_claim_transfers: returned_claim_transfers.clone(),
            returned_claims: returned_claims.clone(),
        },
    }))
}

fn direct_plan(target: NativeTarget, shape: ValueShape) -> Result<CallPlan, LoweringError> {
    evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: Some(shape),
        },
    )
    .map_err(LoweringError::AbiPlan)
}

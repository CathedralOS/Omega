//! Boundary result-home and runtime scalar-argument assignment.

use std::collections::BTreeMap;

use super::{
    AssignedBoundaryResult, AssignedNormalizedForeignScalarArgument, AssignedStructuralHome,
    AssignedUnitScalarHome, AssignmentError, CallSignature, CallingPolicy, MachineRegister,
    NativeTarget, OperationId, PlaceId, TargetUnitOperation, TargetUnitScalarArgumentSource,
    ValueId, ValueLocation, ValueShape, evaluate_call_plan, scalar_call,
};

pub(super) fn assign_result(
    operation: OperationId,
    result: &target_operations::TargetBoundaryResult,
    assigned_homes: &mut BTreeMap<PlaceId, AssignedStructuralHome>,
    next_home: &mut u32,
) -> Result<AssignedBoundaryResult, AssignmentError> {
    let target_operations::TargetBoundaryResult::Structural(requirement) = result else {
        return Ok(AssignedBoundaryResult::Unit);
    };
    let layout = requirement
        .layout
        .sum()
        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
    if layout.tag_byte_offset != 0 || layout.tag_shape != ValueShape::integer(4, 4) {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    let home =
        super::super::structural_homes::assign(operation, requirement, assigned_homes, next_home)?;
    Ok(AssignedBoundaryResult::Structural(home))
}

pub(super) fn assign_runtime_scalar_arguments(
    target: NativeTarget,
    arguments: &[target_operations::TargetUnitScalarCallArgument],
    preceding_operations: &[TargetUnitOperation],
    assigned_homes: &BTreeMap<ValueId, AssignedUnitScalarHome>,
) -> Result<Vec<AssignedNormalizedForeignScalarArgument>, AssignmentError> {
    let [argument] = arguments else {
        return if arguments.is_empty() {
            Ok(Vec::new())
        } else {
            Err(AssignmentError::ExpressionStackFrameNotEncodable)
        };
    };
    let semantic_vocabulary::ScalarType::Integer(integer_type) = argument.scalar_type() else {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    };
    let shape = scalar_call::fixed_integer_shape(argument.source_value(), integer_type)?;
    let plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .map_err(|_| AssignmentError::ExpressionStackFrameNotEncodable)?;
    if argument.parameter_index != 0
        || plan.parameters.as_slice() != [argument.placement.clone()]
        || argument.placement.shape != shape
    {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    let source = match argument.source {
        TargetUnitScalarArgumentSource::Parameter { .. } => {
            return Err(AssignmentError::ExpressionStackFrameNotEncodable);
        }
        source => scalar_call::assign_known_unit_scalar_source(
            source,
            preceding_operations,
            assigned_homes,
        )
        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?,
    };
    let scratch = match target.architecture {
        target::Architecture::X86_64 => MachineRegister::X86R11,
        target::Architecture::Aarch64 => MachineRegister::Aarch64X(9),
    };
    Ok(vec![AssignedNormalizedForeignScalarArgument {
        parameter_index: 0,
        source,
        placement: calling_conventions::ValuePlacement {
            shape,
            locations: vec![ValueLocation::Register {
                register: scratch,
                value_byte_offset: 0,
                byte_size: shape.byte_size,
            }],
        },
    }])
}

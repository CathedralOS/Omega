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
    result: &omega_target_operations::TargetBoundaryResult,
    assigned_homes: &mut BTreeMap<PlaceId, AssignedStructuralHome>,
    next_home: &mut u32,
) -> Result<AssignedBoundaryResult, AssignmentError> {
    let omega_target_operations::TargetBoundaryResult::Structural(requirement) = result else {
        return Ok(AssignedBoundaryResult::Unit);
    };
    if requirement.defining_operation != operation
        || requirement.layout.tag_byte_offset != 0
        || requirement.layout.tag_shape != ValueShape::integer(4, 4)
        || requirement.layout.shape.byte_size == 0
        || requirement.layout.shape.alignment == 0
    {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    *next_home = scalar_call::align_unit_frame_offset(
        *next_home,
        u32::from(requirement.layout.shape.alignment),
    )?;
    let home = AssignedStructuralHome {
        requirement: requirement.clone(),
        byte_offset: *next_home,
    };
    *next_home = next_home
        .checked_add(u32::from(requirement.layout.shape.byte_size))
        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
    if assigned_homes
        .insert(requirement.result.place, home.clone())
        .is_some()
    {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    if &home.requirement != requirement {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    Ok(AssignedBoundaryResult::Structural(home))
}

pub(super) fn assign_runtime_scalar_arguments(
    target: NativeTarget,
    arguments: &[omega_target_operations::TargetUnitScalarCallArgument],
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
    let psi_core::ScalarType::Integer(integer_type) = argument.scalar_type() else {
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
        omega_target::Architecture::X86_64 => MachineRegister::X86R11,
        omega_target::Architecture::Aarch64 => MachineRegister::Aarch64X(9),
    };
    Ok(vec![AssignedNormalizedForeignScalarArgument {
        parameter_index: 0,
        source,
        placement: omega_calling_conventions::ValuePlacement {
            shape,
            locations: vec![ValueLocation::Register {
                register: scratch,
                value_byte_offset: 0,
                byte_size: shape.byte_size,
            }],
        },
    }])
}

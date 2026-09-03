//! Direct helper-call emission for an unchanged existential descriptor parameter.

use omega_assigned_target_operations::{AssignedFunction, AssignedOperation};
use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape};
use omega_machine_code::{
    Aarch64ReturnLinkEvidence, ForwardedDynamicParameterCallRecord,
    ForwardedDynamicParameterCallStackEvidence, InternalCallRelocation, ScalarCallStackEvidence,
    UnitCallStackEvidence, UnitStackEvidence,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::CallSiteOwner;

use crate::{
    EmissionError, aarch64_load_base, aarch64_store_base, aarch64_unit_stack_access,
    append_aarch64_instructions, emit_aarch64_adjust_sp, emit_x86_64_adjust_sp,
    stack_adjustment_pair,
};

pub(super) struct EmittedForwardedDynamicParameterCall {
    pub bytes: Vec<u8>,
    pub record: ForwardedDynamicParameterCallRecord,
    pub relocation: InternalCallRelocation,
    pub unit_stack: Option<UnitStackEvidence>,
    pub unit_affine_cleanup: Option<omega_machine_code::UnitAffineCleanupRecord>,
    pub return_offset: usize,
    pub return_byte_count: usize,
}

pub(super) fn emit(
    function: &AssignedFunction,
    target: NativeTarget,
    functions: &[AssignedFunction],
) -> Result<EmittedForwardedDynamicParameterCall, EmissionError> {
    let (
        psi_edge,
        psi_operation,
        scalar_result,
        callee,
        argument,
        parameter_abi,
        instance_destination,
        table_destination,
        function_call_plan,
        callee_call_plan,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    ) = match &function.operation {
        AssignedOperation::ReturnForwardedDynamicParameterScalarCall {
            psi_edge,
            psi_operation,
            source_value,
            scalar_type,
            callee,
            argument,
            parameter_abi,
            instance_destination,
            table_destination,
            function_call_plan,
            callee_call_plan,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => (
            *psi_edge,
            *psi_operation,
            Some((*source_value, *scalar_type)),
            *callee,
            argument,
            parameter_abi,
            *instance_destination,
            *table_destination,
            function_call_plan,
            callee_call_plan,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        ),
        AssignedOperation::ForwardDynamicParameterUnitCall {
            psi_edge,
            psi_operation,
            callee,
            argument,
            parameter_abi,
            instance_destination,
            table_destination,
            function_call_plan,
            callee_call_plan,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => (
            *psi_edge,
            *psi_operation,
            None,
            *callee,
            argument,
            parameter_abi,
            *instance_destination,
            *table_destination,
            function_call_plan,
            callee_call_plan,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        ),
        _ => unreachable!("forwarded dynamic-parameter emitter receives its exact role"),
    };
    let invalid = || EmissionError::InvalidDynamicDescriptorCallCustody(psi_operation);
    let helper = functions
        .iter()
        .find(|candidate| candidate.machine == callee)
        .ok_or_else(invalid)?;
    let (helper_parameter, helper_call_plan, helper_scalar_type) = match &helper.operation {
        AssignedOperation::ReturnForwardedDynamicParameterScalarCall {
            parameter_abi,
            function_call_plan,
            scalar_type,
            ..
        }
        | AssignedOperation::ReturnDynamicParameterScalarCall {
            parameter_abi,
            function_call_plan,
            scalar_type,
            ..
        } => (
            &parameter_abi.parameter,
            function_call_plan,
            Some(*scalar_type),
        ),
        AssignedOperation::ForwardDynamicParameterUnitCall {
            parameter_abi,
            function_call_plan,
            ..
        }
        | AssignedOperation::DynamicParameterUnitCall {
            parameter_abi,
            function_call_plan,
            ..
        } => (&parameter_abi.parameter, function_call_plan, None),
        _ => return Err(invalid()),
    };
    let result_shape = scalar_result.map(|(_, scalar_type)| match scalar_type {
        psi_core::ScalarType::Boolean => ValueShape::integer(1, 1),
        psi_core::ScalarType::Integer(integer) => ValueShape::integer(
            integer.bits().div_ceil(8),
            integer.bits().div_ceil(8).next_power_of_two().min(8),
        ),
        psi_core::ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32) => {
            ValueShape::float(4)
        }
        psi_core::ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary64) => {
            ValueShape::float(8)
        }
    });
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    let signature = CallSignature {
        parameters: vec![
            ValueShape::integer(pointer_size, pointer_alignment),
            ValueShape::integer(pointer_size, pointer_alignment),
        ],
        result: result_shape,
    };
    if helper.attachment.is_some()
        || helper_parameter != &argument.target
        || helper_call_plan != callee_call_plan
        || helper_scalar_type != scalar_result.map(|(_, scalar_type)| scalar_type)
        || !argument.has_complete_custody(function.machine, psi_operation, callee)
        || !matches!(
            &argument.source,
            omega_target_operations::AbstractDynamicDescriptorSource::Parameter(source)
                if source == &parameter_abi.parameter
        )
        || function_call_plan.policy != CallingPolicy::native_for_target(target)
        || callee_call_plan.policy != CallingPolicy::native_for_target(target)
        || omega_calling_conventions::validate_call_plan(function_call_plan, &signature).is_err()
        || omega_calling_conventions::validate_call_plan(callee_call_plan, &signature).is_err()
        || function_call_plan != callee_call_plan
        || parameter_abi.instance != instance_destination
        || parameter_abi.table != table_destination
        || parameter_abi.instance == parameter_abi.table
        || !claim_transfers.is_empty()
        || !requirement_obligations.is_empty()
        || !crash_continuations.is_empty()
    {
        return Err(invalid());
    }

    let (bytes, mut relocation, call_start, call_count, raw_stack, return_offset, return_count) =
        match target.architecture {
            Architecture::X86_64 => emit_x86_64(
                psi_operation,
                callee,
                u32::from(callee_call_plan.shadow_bytes),
            )?,
            Architecture::Aarch64 => emit_aarch64(psi_operation, callee)?,
        };
    let (call_stack, unit_stack) = if scalar_result.is_some() {
        (
            ForwardedDynamicParameterCallStackEvidence::Scalar(raw_stack),
            None,
        )
    } else {
        let (call_stack, unit_stack) = match target.architecture {
            Architecture::X86_64 => (
                UnitCallStackEvidence {
                    outbound: raw_stack.outbound,
                },
                UnitStackEvidence {
                    frame: None,
                    aarch64_return_link: None,
                    stack_alignment: 16,
                },
            ),
            Architecture::Aarch64 => (
                UnitCallStackEvidence { outbound: None },
                UnitStackEvidence {
                    frame: raw_stack.outbound,
                    aarch64_return_link: raw_stack.aarch64_return_link,
                    stack_alignment: 16,
                },
            ),
        };
        relocation.scalar_stack = None;
        relocation.unit_stack = Some(call_stack);
        (
            ForwardedDynamicParameterCallStackEvidence::Unit(call_stack),
            Some(unit_stack),
        )
    };
    Ok(EmittedForwardedDynamicParameterCall {
        unit_affine_cleanup: unit_stack.map(|_| omega_machine_code::UnitAffineCleanupRecord {
            psi_edge,
            structural_types: Vec::new(),
            locals: Vec::new(),
            actions: Vec::new(),
            code_offset: return_offset,
            byte_count: return_count,
        }),
        record: ForwardedDynamicParameterCallRecord {
            psi_edge,
            psi_operation,
            source_value: scalar_result.map(|(source_value, _)| source_value),
            scalar_type: scalar_result.map(|(_, scalar_type)| scalar_type),
            callee,
            argument: argument.clone(),
            parameter: parameter_abi.parameter.clone(),
            function_call_plan: function_call_plan.clone(),
            callee_call_plan: callee_call_plan.clone(),
            instance: parameter_abi.instance,
            table: parameter_abi.table,
            instance_destination,
            table_destination,
            direct_call_offset: call_start,
            direct_call_byte_count: call_count,
            call_stack,
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: return_offset,
        },
        bytes,
        relocation,
        unit_stack,
        return_offset,
        return_byte_count: return_count,
    })
}

#[allow(clippy::type_complexity)]
fn emit_x86_64(
    operation: psi_core::OperationId,
    callee: psi_core::MachineId,
    shadow_bytes: u32,
) -> Result<
    (
        Vec<u8>,
        InternalCallRelocation,
        usize,
        usize,
        ScalarCallStackEvidence,
        usize,
        usize,
    ),
    EmissionError,
> {
    let padding = (8 + 16 - (shadow_bytes % 16)) % 16;
    let call_stack_bytes = shadow_bytes
        .checked_add(padding)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    let mut bytes = Vec::new();
    let allocation = if call_stack_bytes == 0 {
        None
    } else {
        let offset = bytes.len();
        emit_x86_64_adjust_sp(&mut bytes, call_stack_bytes, false);
        Some((offset, bytes.len() - offset))
    };
    let call_start = bytes.len();
    bytes.push(0xe8);
    let relocation_offset = bytes.len();
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    let release = if call_stack_bytes == 0 {
        None
    } else {
        let offset = bytes.len();
        emit_x86_64_adjust_sp(&mut bytes, call_stack_bytes, true);
        Some((offset, bytes.len() - offset))
    };
    let call_stack = ScalarCallStackEvidence {
        outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        aarch64_return_link: None,
    };
    let return_offset = bytes.len();
    bytes.push(0xc3);
    Ok((
        bytes,
        InternalCallRelocation {
            owner: CallSiteOwner::Operation(operation),
            target: callee,
            unit_stack: None,
            scalar_stack: Some(call_stack),
            offset: relocation_offset,
        },
        call_start,
        5,
        call_stack,
        return_offset,
        1,
    ))
}

#[allow(clippy::type_complexity)]
fn emit_aarch64(
    operation: psi_core::OperationId,
    callee: psi_core::MachineId,
) -> Result<
    (
        Vec<u8>,
        InternalCallRelocation,
        usize,
        usize,
        ScalarCallStackEvidence,
        usize,
        usize,
    ),
    EmissionError,
> {
    let mut instructions = Vec::new();
    let allocation_offset = 0;
    emit_aarch64_adjust_sp(&mut instructions, 16, false)?;
    let link_store_offset = instructions.len() * 4;
    instructions.push(aarch64_unit_stack_access(aarch64_store_base(8)?, 30, 0, 8)?);
    let call_offset = instructions.len() * 4;
    instructions.push(0x9400_0000);
    let link_load_offset = instructions.len() * 4;
    instructions.push(aarch64_unit_stack_access(aarch64_load_base(8)?, 30, 0, 8)?);
    let release_offset = instructions.len() * 4;
    emit_aarch64_adjust_sp(&mut instructions, 16, true)?;
    let return_offset = instructions.len() * 4;
    instructions.push(0xd65f_03c0);
    let call_stack = ScalarCallStackEvidence {
        outbound: stack_adjustment_pair(
            16,
            Some((allocation_offset, 4)),
            Some((release_offset, 4)),
        ),
        aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
            frame_byte_offset: 0,
            store_offset: link_store_offset,
            load_offset: link_load_offset,
        }),
    };
    let mut bytes = Vec::with_capacity(instructions.len() * 4);
    append_aarch64_instructions(&mut bytes, instructions);
    Ok((
        bytes,
        InternalCallRelocation {
            owner: CallSiteOwner::Operation(operation),
            target: callee,
            unit_stack: None,
            scalar_stack: Some(call_stack),
            offset: call_offset,
        },
        call_offset,
        4,
        call_stack,
        return_offset,
        4,
    ))
}

//! Forwarded existential-descriptor parameter-call emission.

use omega_assigned_target_operations::{AssignedDynamicParameterCallMechanism, AssignedOperation};
use omega_machine_code::{
    Aarch64ReturnLinkEvidence, DynamicParameterCallMechanismRecord, DynamicParameterCallRecord,
    ScalarCallStackEvidence,
};
use omega_target::{Architecture, NativeTarget};

use crate::{
    EmissionError, aarch64_load_base, aarch64_unit_memory_access, aarch64_unit_stack_access,
    append_aarch64_instructions, emit_aarch64_adjust_sp, emit_x86_64_adjust_sp,
    stack_adjustment_pair, x86_unit_register,
};

pub(super) struct EmittedDynamicParameterCall {
    pub bytes: Vec<u8>,
    pub record: DynamicParameterCallRecord,
    pub return_offset: usize,
    pub return_byte_count: usize,
}

pub(super) fn emit(
    operation: &AssignedOperation,
    owner: psi_core::MachineId,
    target: NativeTarget,
) -> Result<EmittedDynamicParameterCall, EmissionError> {
    let (
        psi_edge,
        psi_operation,
        scalar_result,
        parameter_abi,
        requirement,
        function_call_plan,
        dispatch_call_plan,
        table_slot_byte_offset,
        mechanism,
    ) = match operation {
        AssignedOperation::ReturnDynamicParameterScalarCall {
            psi_edge,
            psi_operation,
            source_value,
            scalar_type,
            parameter_abi,
            requirement,
            function_call_plan,
            dispatch_call_plan,
            table_slot_byte_offset,
            mechanism,
        } => (
            *psi_edge,
            *psi_operation,
            Some((*source_value, *scalar_type)),
            parameter_abi,
            requirement,
            function_call_plan,
            dispatch_call_plan,
            *table_slot_byte_offset,
            mechanism,
        ),
        AssignedOperation::DynamicParameterUnitCall {
            psi_edge,
            psi_operation,
            parameter_abi,
            requirement,
            function_call_plan,
            dispatch_call_plan,
            table_slot_byte_offset,
            mechanism,
        } => (
            *psi_edge,
            *psi_operation,
            None,
            parameter_abi,
            requirement,
            function_call_plan,
            dispatch_call_plan,
            *table_slot_byte_offset,
            mechanism,
        ),
        _ => unreachable!("dynamic-parameter emitter receives only its exact role"),
    };
    let invalid = || EmissionError::InvalidDynamicParameterCallCustody(psi_operation);
    if parameter_abi.parameter.owner != owner
        || parameter_abi.parameter.ordinal != 0
        || parameter_abi
            .parameter
            .requirements
            .get(usize::try_from(requirement.slot).map_err(|_| invalid())?)
            != Some(requirement)
        || requirement.slot.checked_mul(8) != Some(table_slot_byte_offset)
        || function_call_plan.result != dispatch_call_plan.result
        || dispatch_call_plan.parameters.len() != 1
        || function_call_plan.parameters.len() != 2
        || function_call_plan.shadow_bytes != dispatch_call_plan.shadow_bytes
    {
        return Err(invalid());
    }

    let (bytes, call_offset, call_byte_count, call_stack, physical_mechanism) =
        match (*mechanism, target.architecture) {
            (
                AssignedDynamicParameterCallMechanism::X86MemoryIndirect { table },
                Architecture::X86_64,
            ) if table == parameter_abi.table => emit_x86_64(
                table,
                table_slot_byte_offset,
                u32::from(dispatch_call_plan.shadow_bytes),
            )?,
            (
                AssignedDynamicParameterCallMechanism::Aarch64LoadedIndirect {
                    table,
                    target: call_target,
                },
                Architecture::Aarch64,
            ) if table == parameter_abi.table => {
                emit_aarch64(table, call_target, table_slot_byte_offset)?
            }
            _ => return Err(invalid()),
        };
    let return_byte_count = match target.architecture {
        Architecture::X86_64 => 1,
        Architecture::Aarch64 => 4,
    };
    let return_offset = bytes.len() - return_byte_count;
    Ok(EmittedDynamicParameterCall {
        record: DynamicParameterCallRecord {
            psi_edge,
            psi_operation,
            source_value: scalar_result.map(|result| result.0),
            scalar_type: scalar_result.map(|result| result.1),
            parameter: parameter_abi.parameter.clone(),
            requirement: requirement.clone(),
            function_call_plan: function_call_plan.clone(),
            dispatch_call_plan: dispatch_call_plan.clone(),
            instance: parameter_abi.instance,
            table: parameter_abi.table,
            table_slot_byte_offset,
            mechanism: physical_mechanism,
            indirect_call_offset: call_offset,
            indirect_call_byte_count: call_byte_count,
            call_stack,
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: return_offset,
        },
        bytes,
        return_offset,
        return_byte_count,
    })
}

fn emit_x86_64(
    table: omega_target_operations::MachineRegister,
    slot_offset: u32,
    shadow_bytes: u32,
) -> Result<
    (
        Vec<u8>,
        usize,
        usize,
        ScalarCallStackEvidence,
        DynamicParameterCallMechanismRecord,
    ),
    EmissionError,
> {
    let table_code = x86_unit_register(table)?;
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
    let call_offset = bytes.len();
    if table_code >= 8 {
        bytes.push(0x41); // REX.B
    }
    bytes.push(0xff);
    if table_code & 7 == 4 {
        bytes.push(0x94); // CALL qword ptr [r12/rsp + disp32]
        bytes.push(0x24);
    } else {
        bytes.push(0x90 | (table_code & 7)); // CALL qword ptr [base + disp32]
    }
    bytes.extend_from_slice(&slot_offset.to_le_bytes());
    let call_byte_count = bytes.len() - call_offset;
    let release = if call_stack_bytes == 0 {
        None
    } else {
        let offset = bytes.len();
        emit_x86_64_adjust_sp(&mut bytes, call_stack_bytes, true);
        Some((offset, bytes.len() - offset))
    };
    bytes.push(0xc3);
    Ok((
        bytes,
        call_offset,
        call_byte_count,
        ScalarCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
            aarch64_return_link: None,
        },
        DynamicParameterCallMechanismRecord::X86MemoryIndirect { table },
    ))
}

fn emit_aarch64(
    table: omega_target_operations::MachineRegister,
    call_target: omega_target_operations::MachineRegister,
    slot_offset: u32,
) -> Result<
    (
        Vec<u8>,
        usize,
        usize,
        ScalarCallStackEvidence,
        DynamicParameterCallMechanismRecord,
    ),
    EmissionError,
> {
    let table_code = crate::aarch64_unit_register(table)?;
    let target_code = crate::aarch64_unit_register(call_target)?;
    let mut instructions = Vec::new();
    let allocation_offset = 0;
    emit_aarch64_adjust_sp(&mut instructions, 16, false)?;
    let link_store_offset = instructions.len() * 4;
    instructions.push(aarch64_unit_stack_access(
        crate::aarch64_store_base(8)?,
        30,
        0,
        8,
    )?);
    instructions.push(aarch64_unit_memory_access(
        aarch64_load_base(8)?,
        target_code,
        table_code,
        slot_offset,
        8,
    )?);
    let call_offset = instructions.len() * 4;
    instructions.push(0xd63f_0000 | (u32::from(target_code) << 5)); // blr xN
    let link_load_offset = instructions.len() * 4;
    instructions.push(aarch64_unit_stack_access(aarch64_load_base(8)?, 30, 0, 8)?);
    let release_offset = instructions.len() * 4;
    emit_aarch64_adjust_sp(&mut instructions, 16, true)?;
    instructions.push(0xd65f_03c0); // ret
    let mut bytes = Vec::with_capacity(instructions.len() * 4);
    append_aarch64_instructions(&mut bytes, instructions);
    Ok((
        bytes,
        call_offset,
        4,
        ScalarCallStackEvidence {
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
        },
        DynamicParameterCallMechanismRecord::Aarch64LoadedIndirect {
            table,
            target: call_target,
        },
    ))
}

#[cfg(test)]
mod tests {
    use omega_assigned_target_operations::{
        AssignedDynamicDescriptorParameterAbi, AssignedDynamicParameterCallMechanism,
        AssignedFunction, AssignedOperation, AssignedOperationPlan,
    };
    use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    use omega_target::NativeTarget;
    use omega_target_operations::MachineRegister;
    use psi_core::{EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ScalarType, ValueId};
    use psi_terminal::{
        ClosedConformanceCallableResult, SemanticFingerprint, StructuralAccess,
        TerminalDynamicDescriptorParameter, TerminalDynamicRequirement, TerminalPsiIdentity,
        VocabularyMarker,
    };

    use super::emit;

    fn operation(target: NativeTarget) -> AssignedOperation {
        let pointer = ValueShape::integer(8, 8);
        let result = ValueShape::integer(4, 4);
        let function_call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![pointer, pointer],
                result: Some(result),
            },
        )
        .expect("descriptor helper ABI");
        let dispatch_call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![pointer],
                result: Some(result),
            },
        )
        .expect("erased adapter ABI");
        let machine = MachineId::new(1).unwrap();
        let requirement = TerminalDynamicRequirement {
            slot: 0,
            declaring_trait_identity: "Measure".into(),
            public_requirement_identity: "Measure::measure".into(),
            result: ClosedConformanceCallableResult::I32,
        };
        let parameter = TerminalDynamicDescriptorParameter {
            owner: machine,
            ordinal: 0,
            source_position: 0,
            trait_identity: "Measure".into(),
            access: StructuralAccess::SharedBorrow,
            requirements: vec![requirement.clone()],
        };
        let instance = match target.architecture {
            omega_target::Architecture::X86_64 => {
                if target.object_format == omega_target::ObjectFormat::Coff {
                    MachineRegister::X86Rcx
                } else {
                    MachineRegister::X86Rdi
                }
            }
            omega_target::Architecture::Aarch64 => MachineRegister::Aarch64X(0),
        };
        let table = match target.architecture {
            omega_target::Architecture::X86_64 => {
                if target.object_format == omega_target::ObjectFormat::Coff {
                    MachineRegister::X86Rdx
                } else {
                    MachineRegister::X86Rsi
                }
            }
            omega_target::Architecture::Aarch64 => MachineRegister::Aarch64X(1),
        };
        let mechanism = match target.architecture {
            omega_target::Architecture::X86_64 => {
                AssignedDynamicParameterCallMechanism::X86MemoryIndirect { table }
            }
            omega_target::Architecture::Aarch64 => {
                AssignedDynamicParameterCallMechanism::Aarch64LoadedIndirect {
                    table,
                    target: MachineRegister::Aarch64X(16),
                }
            }
        };
        AssignedOperation::ReturnDynamicParameterScalarCall {
            psi_edge: EdgeId::new(1).unwrap(),
            psi_operation: OperationId::new(1).unwrap(),
            source_value: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
            parameter_abi: AssignedDynamicDescriptorParameterAbi {
                parameter,
                instance,
                table,
            },
            requirement,
            function_call_plan,
            dispatch_call_plan,
            table_slot_byte_offset: 0,
            mechanism,
        }
    }

    #[test]
    fn x86_64_emits_memory_indirect_slot_call_with_balanced_stack() {
        let emitted = emit(
            &operation(NativeTarget::linux_x64()),
            MachineId::new(1).unwrap(),
            NativeTarget::linux_x64(),
        )
        .expect("emit forwarded descriptor helper");
        assert_eq!(
            emitted.bytes,
            [
                0x48, 0x83, 0xec, 0x08, // sub rsp, 8
                0xff, 0x96, 0, 0, 0, 0, // call [rsi + 0]
                0x48, 0x83, 0xc4, 0x08, // add rsp, 8
                0xc3, // ret
            ]
        );
        assert_eq!(emitted.record.indirect_call_offset, 4);
        assert_eq!(emitted.record.indirect_call_byte_count, 6);
        assert_eq!(emitted.record.byte_count, 14);
        assert_eq!(emitted.return_offset, 14);
    }

    #[test]
    fn aarch64_preserves_the_incoming_link_around_loaded_slot_call() {
        let emitted = emit(
            &operation(NativeTarget::linux_arm64()),
            MachineId::new(1).unwrap(),
            NativeTarget::linux_arm64(),
        )
        .expect("emit forwarded descriptor helper");
        assert_eq!(emitted.bytes.len(), 28);
        assert_eq!(emitted.record.indirect_call_offset, 12);
        assert_eq!(emitted.record.indirect_call_byte_count, 4);
        assert_eq!(emitted.record.byte_count, 24);
        assert_eq!(emitted.return_offset, 24);
        let link = emitted
            .record
            .call_stack
            .aarch64_return_link
            .expect("nested call preserves x30");
        assert_eq!(link.frame_byte_offset, 0);
        assert_eq!(link.store_offset, 4);
        assert_eq!(link.load_offset, 16);
    }

    #[test]
    fn complete_machine_emission_retains_role_specific_call_and_attribution() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let machine = MachineId::new(1).unwrap();
            let plan = AssignedOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
                },
                target,
                entry: machine,
                functions: vec![AssignedFunction {
                    machine,
                    attachment: None,
                    fixed_integer_scalar_abi: None,
                    mixed_structural_scalar_abi: None,
                    provenance: omega_target_operations::TerminalPsiProvenance {
                        operations: vec![OperationId::new(1).unwrap()],
                        edges: vec![EdgeId::new(1).unwrap()],
                    },
                    operation: operation(target),
                }],
            };
            let emitted = crate::emit_machine_code(&plan).expect("emit helper machine");
            let [function] = emitted.functions.as_slice() else {
                panic!("one helper function")
            };
            assert_eq!(function.dynamic_parameter_calls.len(), 1);
            assert_eq!(function.semantic_code_attribution.len(), 2);
            assert!(function.scalar_stack.is_some());
            assert!(function.dynamic_calls.is_empty());
            assert!(function.stored_dynamic_calls.is_empty());
        }
    }
}

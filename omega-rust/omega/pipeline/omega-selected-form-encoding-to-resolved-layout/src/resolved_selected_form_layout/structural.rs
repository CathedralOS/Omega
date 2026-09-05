use omega_isa_x86_64::{
    X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET,
    X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH,
    X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT,
};
use omega_machine_code::{
    X86_64StructuralUnitInternalControlFixupKind, X86_64StructuralUnitInternalControlFixupState,
};
use omega_post_allocation_machine_to_selected_form_encoding::{
    SelectedFormEncodingState, SelectedFormMachineDisposition,
    SelectedStructuralUnitFunctionEncoding,
};

use super::error::OptimizedResolvedSelectedFormLayoutError;
use super::model::{
    ResolvedSelectedFormRow, ResolvedStructuralUnitCallLayout, ResolvedStructuralUnitFunctionLayout,
};

pub(super) fn layout_structural_unit_function(
    pre: &SelectedStructuralUnitFunctionEncoding,
) -> Result<ResolvedStructuralUnitFunctionLayout, OptimizedResolvedSelectedFormLayoutError> {
    let call = pre
        .call
        .as_ref()
        .map(layout_structural_unit_call)
        .transpose()?;
    let return_offset = match &call {
        None => 0,
        Some(call) => u64::try_from(call.bytes.len())
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?,
    };
    let SelectedFormEncodingState::Encoded { bytes, .. } = &pre.return_instruction.state else {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::StructuralReturnRosterMismatch(
                pre.return_instruction.instruction,
            ),
        );
    };
    if pre.return_instruction.machine_disposition != SelectedFormMachineDisposition::RetainedV1
        || pre.return_instruction.alternative.family
            != omega_selected_instructions::MachineAlternativeFamily::ReturnUnit
        || bytes.as_slice() != [0xc3]
    {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::StructuralReturnRosterMismatch(
                pre.return_instruction.instruction,
            ),
        );
    }
    let byte_count = return_offset
        .checked_add(1)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    if byte_count
        != if call.is_some() {
            u64::try_from(X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT + 1)
                .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?
        } else {
            1
        }
    {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::StructuralEncodingMismatch(
                pre.return_instruction.instruction,
            ),
        );
    }
    Ok(ResolvedStructuralUnitFunctionLayout {
        machine: pre.machine,
        block: pre.block,
        offset: 0,
        byte_count,
        call,
        return_instruction: ResolvedSelectedFormRow {
            instruction: pre.return_instruction.instruction,
            alternative: pre.return_instruction.alternative,
            offset: return_offset,
            bytes: bytes.clone(),
            branch: None,
            internal_machine_fixup: None,
        },
    })
}

fn layout_structural_unit_call(
    pre: &omega_post_allocation_machine_to_selected_form_encoding::SelectedStructuralUnitCallEncodingRow,
) -> Result<ResolvedStructuralUnitCallLayout, OptimizedResolvedSelectedFormLayoutError> {
    let fixup = pre.fixup;
    if pre.bytes.len() != X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT
        || pre.bytes.get(usize::from(X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET)) != Some(&0xe8)
        || pre
            .bytes
            .get(
                usize::from(X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET)
                    ..usize::from(X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET)
                        + usize::from(X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH),
            )
            != Some(&[0, 0, 0, 0][..])
        || fixup.kind
            != X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1
        || fixup.state
            != X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1
        || fixup.callee != pre.callee
        || fixup.opcode_byte_offset != X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET
        || fixup.field_byte_offset != X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET
        || fixup.next_instruction_byte_offset
            != X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET
        || fixup.field_byte_width != X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH
        || fixup.addend != 0
    {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::StructuralEncodingMismatch(pre.instruction),
        );
    }
    Ok(ResolvedStructuralUnitCallLayout {
        instruction: pre.instruction,
        operation: pre.operation,
        callee: pre.callee,
        offset: 0,
        bytes: pre.bytes.clone(),
        footprint: pre.footprint.clone(),
        fixup,
    })
}

#[cfg(test)]
mod tests {
    use super::OptimizedResolvedSelectedFormLayoutError;
    use super::layout_structural_unit_function;
    use omega_calling_conventions::MachineRegister;
    use omega_isa_x86_64::{
        X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET,
        X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET,
        X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH,
        X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT,
    };
    use omega_machine_code::{
        X86_64SelectedStructuralUnitCallFootprint, X86_64StructuralUnitInternalControlFixup,
        X86_64StructuralUnitInternalControlFixupKind,
        X86_64StructuralUnitInternalControlFixupState,
    };
    use omega_selected_instructions::{
        MachineAlternativeKey, MachineEncodedControlEffect, MachineEncodedEffects,
        MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
        SelectedBlockId, SelectedInstructionId,
    };
    use psi_core::{MachineId, OperationId};

    use omega_machine_code::{
        X86_64StructuralUnitArgumentPointerWrite, X86_64StructuralUnitCallerCopyWrite,
        X86_64StructuralUnitRootRead,
    };
    use omega_post_allocation_machine_to_selected_form_encoding::{
        SelectedFormDecodedFootprint, SelectedStructuralUnitCallEncodingRow,
    };
    use omega_post_allocation_machine_to_selected_form_encoding::{
        SelectedFormEncodingRow, SelectedFormEncodingState, SelectedFormMachineDisposition,
        SelectedStructuralUnitFunctionEncoding,
    };
    use omega_selected_instructions::{
        MachineAlternativeFamily, MachineCleanupEffect, MachineTrapBehavior,
        StructuralUnitCallBarrier, StructuralUnitCallEffect,
    };

    fn structural_function(with_call: bool) -> SelectedStructuralUnitFunctionEncoding {
        let machine = MachineId::new(71).unwrap();
        let callee = MachineId::new(72).unwrap();
        let mut bytes = vec![0; X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT];
        bytes[usize::from(X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET)] = 0xe8;
        let call = with_call.then(|| SelectedStructuralUnitCallEncodingRow {
            instruction: SelectedInstructionId(0),
            operation: OperationId::new(81).unwrap(),
            callee,
            bytes,
            footprint: Box::new(X86_64SelectedStructuralUnitCallFootprint {
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: Vec::new(),
                root_reads: [
                    X86_64StructuralUnitRootRead {
                        root: MachineRegister::X86Rcx,
                        byte_offset: 0,
                        byte_count: 8,
                    },
                    X86_64StructuralUnitRootRead {
                        root: MachineRegister::X86Rcx,
                        byte_offset: 8,
                        byte_count: 8,
                    },
                    X86_64StructuralUnitRootRead {
                        root: MachineRegister::X86Rdx,
                        byte_offset: 0,
                        byte_count: 8,
                    },
                    X86_64StructuralUnitRootRead {
                        root: MachineRegister::X86Rdx,
                        byte_offset: 8,
                        byte_count: 8,
                    },
                ],
                caller_copy_writes: [
                    X86_64StructuralUnitCallerCopyWrite {
                        stack_byte_offset: 32,
                        byte_count: 8,
                    },
                    X86_64StructuralUnitCallerCopyWrite {
                        stack_byte_offset: 40,
                        byte_count: 8,
                    },
                    X86_64StructuralUnitCallerCopyWrite {
                        stack_byte_offset: 48,
                        byte_count: 8,
                    },
                    X86_64StructuralUnitCallerCopyWrite {
                        stack_byte_offset: 56,
                        byte_count: 8,
                    },
                ],
                scratch_register_writes: [MachineRegister::X86Rax],
                argument_pointer_writes: [
                    X86_64StructuralUnitArgumentPointerWrite {
                        register: MachineRegister::X86Rcx,
                        stack_byte_offset: 32,
                    },
                    X86_64StructuralUnitArgumentPointerWrite {
                        register: MachineRegister::X86Rdx,
                        stack_byte_offset: 48,
                    },
                ],
                writes_rflags: true,
                frame_byte_count: 72,
                shadow_byte_count: 32,
                pre_call_stack_alignment: 16,
                frame_is_balanced: true,
                trap: MachineTrapBehavior::MayArchitecturalFaultV1,
                barrier: StructuralUnitCallBarrier::CallV1,
                call: StructuralUnitCallEffect::DirectInternalUnitV1,
                cleanup: MachineCleanupEffect::NoneV1,
            }),
            fixup: X86_64StructuralUnitInternalControlFixup {
                kind: X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1,
                state: X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1,
                callee,
                opcode_byte_offset: X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET,
                field_byte_offset: X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET,
                next_instruction_byte_offset: X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET,
                field_byte_width: X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH,
                addend: 0,
            },
        });
        SelectedStructuralUnitFunctionEncoding {
            machine,
            block: SelectedBlockId(0),
            call,
            return_instruction: SelectedFormEncodingRow {
                instruction: SelectedInstructionId(u32::from(with_call)),
                alternative: MachineAlternativeKey {
                    family: MachineAlternativeFamily::ReturnUnit,
                    variant: 0,
                },
                machine_disposition: SelectedFormMachineDisposition::RetainedV1,
                state: SelectedFormEncodingState::Encoded {
                    bytes: vec![0xc3],
                    footprint: Box::new(SelectedFormDecodedFootprint {
                        register_reads: Vec::new(),
                        register_writes: Vec::new(),
                        implicit_defs: Vec::new(),
                        implicit_clobbers: Vec::new(),
                        encoded: MachineEncodedEffects {
                            external_operand_reads: Vec::new(),
                            external_operand_writes: Vec::new(),
                            implicit_unit_uses: Vec::new(),
                            implicit_unit_defs: Vec::new(),
                            implicit_unit_clobbers: Vec::new(),
                            memory: MachineEncodedMemoryEffect::NoneV1,
                            stack: MachineEncodedStackEffect::UnchangedV1,
                            trap: MachineEncodedTrapBehavior::NeverV1,
                            control: MachineEncodedControlEffect::ReturnFromActivationStackV1,
                        },
                    }),
                },
            },
        }
    }

    #[test]
    fn structural_layout_retains_unresolved_call_and_exact_function_spans() {
        let caller = layout_structural_unit_function(&structural_function(true)).unwrap();
        assert_eq!(caller.offset, 0);
        assert_eq!(caller.byte_count, 90);
        assert_eq!(caller.return_instruction.offset, 89);
        assert_eq!(caller.return_instruction.bytes, [0xc3]);
        let call = caller.call.unwrap();
        assert_eq!(call.offset, 0);
        assert_eq!(call.bytes.len(), 89);
        assert_eq!(&call.bytes[81..85], [0, 0, 0, 0]);
        assert_eq!(
            call.fixup.state,
            X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1
        );

        let leaf = layout_structural_unit_function(&structural_function(false)).unwrap();
        assert_eq!(leaf.offset, 0);
        assert_eq!(leaf.byte_count, 1);
        assert!(leaf.call.is_none());
        assert_eq!(leaf.return_instruction.offset, 0);
        assert_eq!(leaf.return_instruction.bytes, [0xc3]);
    }

    #[test]
    fn structural_layout_rejects_template_fixup_and_return_corruption() {
        let mut corrupted = structural_function(true);
        corrupted.call.as_mut().unwrap().bytes[81] = 1;
        assert!(matches!(
            layout_structural_unit_function(&corrupted),
            Err(
                OptimizedResolvedSelectedFormLayoutError::StructuralEncodingMismatch(
                    SelectedInstructionId(0)
                )
            )
        ));

        let mut corrupted = structural_function(true);
        corrupted
            .call
            .as_mut()
            .unwrap()
            .fixup
            .next_instruction_byte_offset = 84;
        assert!(matches!(
            layout_structural_unit_function(&corrupted),
            Err(
                OptimizedResolvedSelectedFormLayoutError::StructuralEncodingMismatch(
                    SelectedInstructionId(0)
                )
            )
        ));

        let mut corrupted = structural_function(false);
        let SelectedFormEncodingState::Encoded { bytes, .. } =
            &mut corrupted.return_instruction.state
        else {
            unreachable!()
        };
        bytes[0] = 0x90;
        assert!(matches!(
            layout_structural_unit_function(&corrupted),
            Err(
                OptimizedResolvedSelectedFormLayoutError::StructuralReturnRosterMismatch(
                    SelectedInstructionId(0)
                )
            )
        ));
    }
}

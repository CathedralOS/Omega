use super::*;

pub(super) fn decode_structural_function(
    cursor: &mut Cursor<'_>,
    allow_i64_less_than: bool,
) -> Result<StructuralUnitFunctionMachineEffects, PreAllocationMachineEffectDecodeError> {
    let machine = decode_machine(cursor)?;
    let block = SelectedBlockId(cursor.u32()?);
    let call = match cursor.byte()? {
        0 => None,
        1 => Some(decode_structural_call(cursor)?),
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let return_instruction = decode_instruction(cursor, allow_i64_less_than, false, false)?;
    let return_effect = decode_effect_link(cursor)?;
    let return_ownership = decode_ownership(cursor)?;
    Ok(StructuralUnitFunctionMachineEffects {
        machine,
        block,
        call,
        return_instruction,
        return_effect,
        return_ownership,
    })
}

pub fn decode_structural_call(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralUnitCallMachineEffects, PreAllocationMachineEffectDecodeError> {
    let instruction = SelectedInstructionId(cursor.u32()?);
    let operation = OperationId::new(cursor.u64()?)
        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
    let callee = decode_machine(cursor)?;
    let constraint = decode_constraint_key(cursor)?;
    let unit_uses = decode_units(cursor)?;
    let unit_defs = decode_units(cursor)?;
    let unit_clobbers = decode_units(cursor)?;
    let layout = decode_structural_layout(cursor)?;
    let effect = decode_effect_link(cursor)?;
    let ownership = decode_ownership(cursor)?;
    let transfer_count = cursor.length()?;
    let mut claim_transfers = Vec::with_capacity(transfer_count.min(cursor.remaining()));
    for _ in 0..transfer_count {
        claim_transfers.push(terminal_psi::ClaimTransfer {
            claim: ClaimId::new(cursor.u64()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
            argument_index: cursor.u32()?,
        });
    }
    let provenance = decode_provenance(cursor)?;
    let declaration = decode_structural_declaration(cursor)?;
    Ok(StructuralUnitCallMachineEffects {
        instruction,
        operation,
        callee,
        constraint,
        unit_uses,
        unit_defs,
        unit_clobbers,
        layout,
        effect,
        ownership,
        claim_transfers,
        provenance,
        declaration,
    })
}

fn decode_structural_layout(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedMicrosoftX64OwnedIndirectPairLayout, PreAllocationMachineEffectDecodeError> {
    let shadow_byte_count = cursor.u32()?;
    let outgoing_frame_byte_count = cursor.u32()?;
    let pre_call_stack_alignment = cursor.u16()?;
    let mut bindings = Vec::with_capacity(2);
    for _ in 0..2 {
        bindings.push(SelectedStructuralUnitIndirectBinding {
            parameter_index: usize::try_from(cursor.u64()?)
                .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)?,
            pointer: decode_machine_register(cursor)?,
            copy_stack_byte_offset: cursor.u32()?,
            byte_count: cursor.u16()?,
            alignment: cursor.u16()?,
        });
    }
    Ok(SelectedMicrosoftX64OwnedIndirectPairLayout {
        shadow_byte_count,
        outgoing_frame_byte_count,
        pre_call_stack_alignment,
        bindings: bindings
            .try_into()
            .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)?,
    })
}

fn decode_machine_register(
    cursor: &mut Cursor<'_>,
) -> Result<target_operations::MachineRegister, PreAllocationMachineEffectDecodeError> {
    use target_operations::MachineRegister as R;
    Ok(match cursor.byte()? {
        0 => R::X86Rax,
        1 => R::X86Rcx,
        2 => R::X86Rdx,
        3 => R::X86Rbx,
        4 => R::X86Rsp,
        5 => R::X86Rbp,
        6 => R::X86Rsi,
        7 => R::X86Rdi,
        8 => R::X86R8,
        9 => R::X86R9,
        10 => R::X86R10,
        11 => R::X86R11,
        12 => R::X86R12,
        13 => R::X86R13,
        14 => R::X86R14,
        15 => R::X86R15,
        16 => R::X86Xmm(cursor.byte()?),
        17 => R::Aarch64X(cursor.byte()?),
        18 => R::Aarch64V(cursor.byte()?),
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    })
}

pub fn decode_effect_link(
    cursor: &mut Cursor<'_>,
) -> Result<EffectLink, PreAllocationMachineEffectDecodeError> {
    Ok(EffectLink {
        input: cursor.u64()?,
        output: cursor.u64()?,
    })
}

fn decode_structural_declaration(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralUnitCallEffectDeclaration, PreAllocationMachineEffectDecodeError> {
    let constraint = decode_constraint_key(cursor)?;
    let memory = match cursor.byte()? {
        1 => StructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
            root_byte_count: cursor.u16()?,
            copy_stack_byte_offsets: [cursor.u32()?, cursor.u32()?],
        },
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let frame = match cursor.byte()? {
        1 => StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
            frame_byte_count: cursor.u32()?,
            shadow_byte_count: cursor.u32()?,
            pre_call_stack_alignment: cursor.u16()?,
        },
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let trap = match cursor.byte()? {
        0 => MachineTrapBehavior::NeverV1,
        1 => MachineTrapBehavior::MayArchitecturalFaultV1,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    if cursor.byte()? != 1 || cursor.byte()? != 1 || cursor.byte()? != 0 {
        return Err(PreAllocationMachineEffectDecodeError::InvalidField);
    }
    Ok(StructuralUnitCallEffectDeclaration {
        constraint,
        memory,
        frame,
        trap,
        barrier: StructuralUnitCallBarrier::CallV1,
        call: StructuralUnitCallEffect::DirectInternalUnitV1,
        cleanup: MachineCleanupEffect::NoneV1,
    })
}

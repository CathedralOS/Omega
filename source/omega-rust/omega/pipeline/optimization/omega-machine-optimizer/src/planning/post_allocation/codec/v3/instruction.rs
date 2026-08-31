//! Physical instruction and operand vocabulary decoding.

use crate::analyses::pre_allocation_effects::codec as effect_codec;
use crate::{PhysicalOperandFootprint, PostAllocationMachineInstruction};
use omega_register_model::{
    RegisterClassId, RegisterOperandAccess, RegisterViewId, RegisterWriteSemantics,
};
use omega_selected_instructions::{SelectedInstructionId, VirtualRegisterId};

use super::super::{
    PostAllocationMachineDecodeError,
    cursor::{byte, decode_units, length, map_field_error, u16_field, u32_field},
};

pub(super) fn decode_instruction(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<PostAllocationMachineInstruction, PostAllocationMachineDecodeError> {
    let instruction = SelectedInstructionId(u32_field(cursor)?);
    let alternative = effect_codec::decode_alternative(cursor).map_err(map_field_error)?;
    let operand_count = length(cursor)?;
    let mut operands = Vec::with_capacity(operand_count.min(cursor.remaining()));
    for _ in 0..operand_count {
        operands.push(decode_operand(cursor)?);
    }
    Ok(PostAllocationMachineInstruction {
        instruction,
        alternative,
        operands,
        implicit_unit_uses: decode_units(cursor)?,
        implicit_unit_defs: decode_units(cursor)?,
        implicit_unit_clobbers: decode_units(cursor)?,
        unit_uses: decode_units(cursor)?,
        unit_defs: decode_units(cursor)?,
        unit_clobbers: decode_units(cursor)?,
    })
}

fn decode_operand(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<PhysicalOperandFootprint, PostAllocationMachineDecodeError> {
    let operand = u16_field(cursor)?;
    let virtual_register = VirtualRegisterId(u32_field(cursor)?);
    let class = RegisterClassId(u16_field(cursor)?);
    let view = RegisterViewId(u16_field(cursor)?);
    let access = match byte(cursor)? {
        0 => RegisterOperandAccess::Use,
        1 => RegisterOperandAccess::Def,
        2 => RegisterOperandAccess::UseDef,
        _ => return Err(PostAllocationMachineDecodeError::InvalidField),
    };
    let storage_units = decode_units(cursor)?;
    let read_units = decode_units(cursor)?;
    let write_units = decode_units(cursor)?;
    let write_semantics = match byte(cursor)? {
        0 => None,
        1 => Some(match byte(cursor)? {
            0 => RegisterWriteSemantics::ExactView,
            1 => RegisterWriteSemantics::PreservesUnwritten,
            2 => RegisterWriteSemantics::ZeroExtendsParent,
            3 => RegisterWriteSemantics::ZeroExtendsWithinUnit,
            4 => RegisterWriteSemantics::Discards,
            5 => RegisterWriteSemantics::InstructionDefined,
            _ => return Err(PostAllocationMachineDecodeError::InvalidField),
        }),
        _ => return Err(PostAllocationMachineDecodeError::InvalidField),
    };
    Ok(PhysicalOperandFootprint {
        operand,
        virtual_register,
        class,
        view,
        access,
        storage_units,
        read_units,
        write_units,
        write_semantics,
    })
}

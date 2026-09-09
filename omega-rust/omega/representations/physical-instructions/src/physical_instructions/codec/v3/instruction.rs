//! Physical instruction and operand vocabulary decoding.

use crate::{PhysicalOperandFootprint, PostAllocationMachineInstruction};
use register_model::{
    RegisterClassId, RegisterOperandAccess, RegisterViewId, RegisterWriteSemantics,
};
use selected_instructions::selected_instructions::effects::program::encoding as effect_codec;
use selected_instructions::{SelectedInstructionId, VirtualRegisterId};

use super::super::{
    PostAllocationMachineDecodeError,
    cursor::{byte, decode_units, length, map_field_error, u16_field, u32_field, u64_field},
};

pub(super) fn decode_instruction(
    cursor: &mut effect_codec::Cursor<'_>,
    allow_i64_less_than: bool,
    allow_scalar_call: bool,
    allow_jump: bool,
) -> Result<PostAllocationMachineInstruction, PostAllocationMachineDecodeError> {
    let instruction = SelectedInstructionId(u32_field(cursor)?);
    let alternative = if allow_jump {
        effect_codec::decode_alternative(cursor)
    } else if allow_scalar_call {
        effect_codec::decode_alternative_without_jump(cursor)
    } else if allow_i64_less_than {
        effect_codec::decode_alternative_without_scalar_call(cursor)
    } else {
        effect_codec::decode_alternative_legacy(cursor)
    }
    .map_err(map_field_error)?;
    let operand_count = length(cursor)?;
    let mut operands = Vec::with_capacity(operand_count.min(cursor.remaining()));
    for _ in 0..operand_count {
        operands.push(decode_operand(cursor)?);
    }
    let address = match byte(cursor)? {
        9 => Some(crate::PhysicalAddressOperation::HostedReadByte {
            slot: effect_codec::decode_local_storage_slot(cursor).map_err(map_field_error)?,
        }),
        5 => Some(crate::PhysicalAddressOperation::HostedWriteByteI32 {
            slot: effect_codec::decode_local_storage_slot(cursor).map_err(map_field_error)?,
        }),
        6 => {
            let base_operand = u16_field(cursor)?;
            let byte_offset = u32_field(cursor)?;
            let byte_size = byte(cursor)?;
            if !matches!(byte_size, 1 | 2 | 4 | 8) {
                return Err(PostAllocationMachineDecodeError::InvalidField);
            }
            Some(crate::PhysicalAddressOperation::Store {
                base_operand,
                byte_offset,
                byte_size,
            })
        }
        7 => Some(crate::PhysicalAddressOperation::AddressOffset {
            base_operand: u16_field(cursor)?,
            byte_offset: u32_field(cursor)?,
        }),
        0 => None,
        4 => Some(crate::PhysicalAddressOperation::Load8Indexed {
            base_operand: u16_field(cursor)?,
            index_operand: u16_field(cursor)?,
        }),
        10 => Some(crate::PhysicalAddressOperation::Load8 {
            base_operand: u16_field(cursor)?,
            byte_offset: u32_field(cursor)?,
        }),
        11 => Some(crate::PhysicalAddressOperation::Load16 {
            base_operand: u16_field(cursor)?,
            byte_offset: u32_field(cursor)?,
        }),
        8 => Some(crate::PhysicalAddressOperation::Load32 {
            base_operand: u16_field(cursor)?,
            byte_offset: u32_field(cursor)?,
        }),
        1 => Some(crate::PhysicalAddressOperation::Load64 {
            base_operand: u16_field(cursor)?,
            byte_offset: u32_field(cursor)?,
        }),
        tag @ (2 | 3) => {
            let slot = match byte(cursor)? {
                2 => selected_instructions::FrameStorageSlotId::Incoming {
                    parameter_index: u32_field(cursor)?,
                    abi_stack_byte_offset: u32_field(cursor)?,
                },
                0 => selected_instructions::FrameStorageSlotId::Outgoing(
                    selected_instructions::OutgoingArgumentSlotId {
                        operation: semantic_vocabulary::OperationId::new(u64_field(cursor)?)
                            .ok_or(PostAllocationMachineDecodeError::InvalidField)?,
                        argument_index: u32_field(cursor)?,
                    },
                ),
                1 => selected_instructions::FrameStorageSlotId::Local(
                    effect_codec::decode_local_storage_slot(cursor).map_err(map_field_error)?,
                ),
                _ => return Err(PostAllocationMachineDecodeError::InvalidField),
            };
            let byte_offset = u32_field(cursor)?;
            Some(if tag == 2 {
                crate::PhysicalAddressOperation::Store64 { slot, byte_offset }
            } else {
                crate::PhysicalAddressOperation::FrameAddress { slot, byte_offset }
            })
        }
        _ => return Err(PostAllocationMachineDecodeError::InvalidField),
    };
    Ok(PostAllocationMachineInstruction {
        instruction,
        alternative,
        operands,
        address,
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

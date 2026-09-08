use register_model::{RegisterClassId, RegisterViewId};
use selected_instructions::{
    SelectedInstructionId, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::ValueId;

use crate::FixedViewCopyDecodeError;

use crate::rewrites::allocation_recovery::fixed_view_copy::codec::{
    primitives::{Cursor, decode_id, decode_option_u16, encode_option_u16, length},
    values::{decode_definition_site, decode_scalar, encode_definition_site, encode_scalar},
};

pub(super) fn encode_register(bytes: &mut Vec<u8>, register: &VirtualRegister) {
    bytes.extend_from_slice(&register.id.0.to_le_bytes());
    encode_scalar(bytes, register.scalar_type);
    bytes.extend_from_slice(&register.class.0.to_le_bytes());
    match register.origin {
        VirtualRegisterOrigin::SpillAddress {
            instruction,
            register,
        } => {
            bytes.push(6);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&register.0.to_le_bytes());
        }

        VirtualRegisterOrigin::StructuralParameter {
            place,
            parameter_index,
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&place.get().to_le_bytes());
            length(bytes, parameter_index);
        }
        VirtualRegisterOrigin::AbiTransport {
            instruction,
            place,
            byte_offset,
        } => {
            bytes.push(5);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&place.get().to_le_bytes());
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        VirtualRegisterOrigin::BlockParameter {
            source_value,
            block,
            parameter_index,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            bytes.extend_from_slice(&block.0.to_le_bytes());
            length(bytes, parameter_index);
        }
        VirtualRegisterOrigin::EntryParameter {
            source_value,
            parameter_index,
        } => {
            bytes.push(0);
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            length(bytes, parameter_index);
        }
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
        }
    }
    match register.definition_site {
        None => bytes.push(0),
        Some(site) => {
            bytes.push(1);
            encode_definition_site(bytes, site);
        }
    }
    encode_option_u16(bytes, register.entry_fixed_view.map(|view| view.0));
}

pub(super) fn decode_register(
    cursor: &mut Cursor<'_>,
) -> Result<VirtualRegister, FixedViewCopyDecodeError> {
    let id = VirtualRegisterId(cursor.u32()?);
    let scalar_type = decode_scalar(cursor)?;
    let class = RegisterClassId(cursor.u16()?);
    let origin = match cursor.byte()? {
        6 => VirtualRegisterOrigin::SpillAddress {
            instruction: SelectedInstructionId(cursor.u32()?),
            register: VirtualRegisterId(cursor.u32()?),
        },
        4 => VirtualRegisterOrigin::StructuralParameter {
            place: decode_id(cursor, semantic_vocabulary::PlaceId::new)?,
            parameter_index: cursor.length()?,
        },
        5 => VirtualRegisterOrigin::AbiTransport {
            instruction: SelectedInstructionId(cursor.u32()?),
            place: decode_id(cursor, semantic_vocabulary::PlaceId::new)?,
            byte_offset: cursor.u32()?,
        },
        3 => VirtualRegisterOrigin::BlockParameter {
            source_value: decode_id(cursor, ValueId::new)?,
            block: selected_instructions::SelectedBlockId(cursor.u32()?),
            parameter_index: cursor.length()?,
        },
        0 => VirtualRegisterOrigin::EntryParameter {
            source_value: decode_id(cursor, ValueId::new)?,
            parameter_index: cursor.length()?,
        },
        1 => VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(cursor.u32()?),
            source_value: decode_id(cursor, ValueId::new)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownRegisterOrigin(tag)),
    };
    Ok(VirtualRegister {
        id,
        scalar_type,
        class,
        origin,
        definition_site: match cursor.byte()? {
            0 => None,
            1 => Some(decode_definition_site(cursor)?),
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        },
        entry_fixed_view: decode_option_u16(cursor)?.map(RegisterViewId),
    })
}

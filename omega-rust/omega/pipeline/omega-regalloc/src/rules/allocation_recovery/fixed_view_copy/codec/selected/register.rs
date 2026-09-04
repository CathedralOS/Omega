use omega_register_model::{RegisterClassId, RegisterViewId};
use omega_selected_instructions::{
    SelectedInstructionId, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use psi_core::ValueId;

use crate::FixedViewCopyDecodeError;

use crate::rules::allocation_recovery::fixed_view_copy::codec::{
    primitives::{Cursor, decode_id, decode_option_u16, encode_option_u16, length},
    values::{decode_definition_site, decode_scalar, encode_definition_site, encode_scalar},
};

pub(super) fn encode_register(bytes: &mut Vec<u8>, register: &VirtualRegister) {
    bytes.extend_from_slice(&register.id.0.to_le_bytes());
    encode_scalar(bytes, register.scalar_type);
    bytes.extend_from_slice(&register.class.0.to_le_bytes());
    match register.origin {
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
        VirtualRegisterOrigin::LegalizationTemporary {
            instruction,
            temporary,
            source_value,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&temporary.0.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
        }
    }
    encode_definition_site(bytes, register.definition_site);
    encode_option_u16(bytes, register.entry_fixed_view.map(|view| view.0));
}

pub(super) fn decode_register(
    cursor: &mut Cursor<'_>,
) -> Result<VirtualRegister, FixedViewCopyDecodeError> {
    let id = VirtualRegisterId(cursor.u32()?);
    let scalar_type = decode_scalar(cursor)?;
    let class = RegisterClassId(cursor.u16()?);
    let origin = match cursor.byte()? {
        0 => VirtualRegisterOrigin::EntryParameter {
            source_value: decode_id(cursor, ValueId::new)?,
            parameter_index: cursor.length()?,
        },
        1 => VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(cursor.u32()?),
            source_value: decode_id(cursor, ValueId::new)?,
        },
        2 => VirtualRegisterOrigin::LegalizationTemporary {
            instruction: SelectedInstructionId(cursor.u32()?),
            temporary: omega_legalized_operations::LegalizedTemporaryId(cursor.u32()?),
            source_value: decode_id(cursor, ValueId::new)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownRegisterOrigin(tag)),
    };
    Ok(VirtualRegister {
        id,
        scalar_type,
        class,
        origin,
        definition_site: decode_definition_site(cursor)?,
        entry_fixed_view: decode_option_u16(cursor)?.map(RegisterViewId),
    })
}

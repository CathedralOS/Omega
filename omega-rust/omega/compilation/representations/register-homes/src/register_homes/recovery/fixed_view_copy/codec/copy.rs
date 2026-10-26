use register_model::RegisterViewId;
use selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use semantic_vocabulary::{MachineId, ValueId};

use crate::{FixedViewCopy, FixedViewCopyDecodeError, FixedViewCopyDestination};

use optimization_unit::encode_value_definition_site_identity;

use register_model::encode_constraint_key;

use super::{
    primitives::{Cursor, decode_id, decode_list, length},
    values::{decode_constraint_key, decode_definition_site, decode_fixed_site},
};

pub(super) fn encode_copy(bytes: &mut Vec<u8>, copy: &FixedViewCopy) {
    crate::codec_bytes::push_encoded(bytes, copy.function.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.machine.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.source_virtual_register.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.source_value.get().to_le_bytes());
    encode_value_definition_site_identity(bytes, copy.source_definition_site);
    crate::codec_bytes::push_encoded(bytes, copy.from_view.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.to_view.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.insertion_block.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.before_instruction.0.to_le_bytes());
    length(bytes, copy.destinations.len());
    for destination in &copy.destinations {
        crate::identity::encode_fixed_constraint_site_identity(bytes, destination.site);
        crate::codec_bytes::push_encoded(bytes, destination.block.0.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, destination.view.0.to_le_bytes());
    }
    crate::codec_bytes::push_encoded(bytes, copy.copy_instruction.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, copy.result_virtual_register.0.to_le_bytes());
    encode_constraint_key(bytes, copy.copy_constraint);
}

pub(super) fn decode_copy(
    cursor: &mut Cursor<'_>,
) -> Result<FixedViewCopy, FixedViewCopyDecodeError> {
    let function = cursor.u32()?;
    let machine = decode_id(cursor, MachineId::new)?;
    let source_virtual_register = VirtualRegisterId(cursor.u32()?);
    let source_value = decode_id(cursor, ValueId::new)?;
    let source_definition_site = decode_definition_site(cursor)?;
    let from_view = RegisterViewId(cursor.u16()?);
    let to_view = RegisterViewId(cursor.u16()?);
    let insertion_block = SelectedBlockId(cursor.u32()?);
    let before_instruction = SelectedInstructionId(cursor.u32()?);
    let destinations = decode_list(cursor, |cursor| {
        Ok(FixedViewCopyDestination {
            site: decode_fixed_site(cursor)?,
            block: SelectedBlockId(cursor.u32()?),
            view: RegisterViewId(cursor.u16()?),
        })
    })?;
    Ok(FixedViewCopy {
        function,
        machine,
        source_virtual_register,
        source_value,
        source_definition_site,
        from_view,
        to_view,
        insertion_block,
        before_instruction,
        destinations,
        copy_instruction: SelectedInstructionId(cursor.u32()?),
        result_virtual_register: VirtualRegisterId(cursor.u32()?),
        copy_constraint: decode_constraint_key(cursor)?,
    })
}

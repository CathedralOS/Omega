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
        VirtualRegisterOrigin::StructuralObservation {
            instruction,
            place,
            byte_offset,
        } => {
            bytes.push(8);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&place.get().to_le_bytes());
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        VirtualRegisterOrigin::ScalarAbiAddress {
            instruction,
            source_value,
        } => {
            bytes.push(7);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
        }
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
        8 => VirtualRegisterOrigin::StructuralObservation {
            instruction: SelectedInstructionId(cursor.u32()?),
            place: decode_id(cursor, semantic_vocabulary::PlaceId::new)?,
            byte_offset: cursor.u32()?,
        },
        7 => VirtualRegisterOrigin::ScalarAbiAddress {
            instruction: SelectedInstructionId(cursor.u32()?),
            source_value: decode_id(cursor, ValueId::new)?,
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_observation_origin_round_trips_without_scalar_definition_site() {
        let source = VirtualRegister {
            id: VirtualRegisterId(3),
            scalar_type: semantic_vocabulary::ScalarType::Boolean,
            class: RegisterClassId(0),
            origin: VirtualRegisterOrigin::StructuralObservation {
                instruction: SelectedInstructionId(5),
                place: semantic_vocabulary::PlaceId::new(7).unwrap(),
                byte_offset: 4,
            },
            definition_site: None,
            entry_fixed_view: None,
        };
        let mut encoded = Vec::new();
        encode_register(&mut encoded, &source);
        assert_eq!(decode_register(&mut Cursor::new(&encoded)).unwrap(), source);
        for end in 0..encoded.len() {
            assert!(decode_register(&mut Cursor::new(&encoded[..end])).is_err());
        }
        for mutation in 0..4 {
            let mut changed = source.clone();
            changed.origin = match mutation {
                0 => VirtualRegisterOrigin::StructuralObservation {
                    instruction: SelectedInstructionId(6),
                    place: semantic_vocabulary::PlaceId::new(7).unwrap(),
                    byte_offset: 4,
                },
                1 => VirtualRegisterOrigin::StructuralObservation {
                    instruction: SelectedInstructionId(5),
                    place: semantic_vocabulary::PlaceId::new(8).unwrap(),
                    byte_offset: 4,
                },
                2 => VirtualRegisterOrigin::StructuralObservation {
                    instruction: SelectedInstructionId(5),
                    place: semantic_vocabulary::PlaceId::new(7).unwrap(),
                    byte_offset: 0,
                },
                _ => VirtualRegisterOrigin::AbiTransport {
                    instruction: SelectedInstructionId(5),
                    place: semantic_vocabulary::PlaceId::new(7).unwrap(),
                    byte_offset: 4,
                },
            };
            let mut changed_bytes = Vec::new();
            encode_register(&mut changed_bytes, &changed);
            assert_ne!(encoded, changed_bytes);
            assert_eq!(
                decode_register(&mut Cursor::new(&changed_bytes)).unwrap(),
                changed
            );
        }
    }
}

//! Canonical virtual-register identity, including exact instruction-local scratch custody.
use super::encode_option_u16;
use super::encode_scalar_type;
use crate::{VirtualRegister, VirtualRegisterOrigin};
use optimization_unit::encode_value_definition_site_identity;

pub(super) fn encode(bytes: &mut Vec<u8>, register: &VirtualRegister) {
    crate::codec_bytes::push_encoded(bytes, register.id.0.to_le_bytes());
    encode_scalar_type(bytes, register.scalar_type);
    crate::codec_bytes::push_encoded(bytes, register.class.0.to_le_bytes());
    match register.origin {
        VirtualRegisterOrigin::InstructionScratch {
            instruction,
            operand,
        } => {
            bytes.push(9);
            crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, operand.to_le_bytes());
        }
        VirtualRegisterOrigin::StructuralObservation {
            instruction,
            place,
            byte_offset,
        } => {
            bytes.push(8);
            crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        VirtualRegisterOrigin::ScalarAbiAddress {
            instruction,
            source_value,
        } => {
            bytes.push(7);
            crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
        }
        VirtualRegisterOrigin::SpillAddress {
            instruction,
            register,
        } => {
            bytes.push(6);
            crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, register.0.to_le_bytes());
        }

        VirtualRegisterOrigin::StructuralParameter {
            place,
            parameter_index,
        } => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, (parameter_index as u64).to_le_bytes());
        }
        VirtualRegisterOrigin::AbiTransport {
            instruction,
            place,
            byte_offset,
        } => {
            bytes.push(5);
            crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }

        VirtualRegisterOrigin::BlockParameter {
            source_value,
            block,
            parameter_index,
        } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, (parameter_index as u64).to_le_bytes());
        }
        VirtualRegisterOrigin::EntryParameter {
            source_value,
            parameter_index,
        } => {
            bytes.push(0);
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, (parameter_index as u64).to_le_bytes());
        }
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
        }
    }
    match register.definition_site {
        Some(site) => {
            bytes.push(1);
            encode_value_definition_site_identity(bytes, site);
        }
        None => bytes.push(0),
    }
    encode_option_u16(bytes, register.entry_fixed_view.map(|view| view.0));
}

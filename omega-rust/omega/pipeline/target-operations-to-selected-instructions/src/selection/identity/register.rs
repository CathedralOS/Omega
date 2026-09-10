//! Canonical virtual-register identity, including exact instruction-local scratch custody.
use super::*;

pub(super) fn encode(bytes: &mut Vec<u8>, register: &VirtualRegister) {
    bytes.extend_from_slice(&register.id.0.to_le_bytes());
    encode_scalar_type(bytes, register.scalar_type);
    bytes.extend_from_slice(&register.class.0.to_le_bytes());
    match register.origin {
        VirtualRegisterOrigin::InstructionScratch {
            instruction,
            operand,
        } => {
            bytes.push(9);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&operand.to_le_bytes());
        }
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
            bytes.extend_from_slice(&(parameter_index as u64).to_le_bytes());
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
            bytes.extend_from_slice(&(parameter_index as u64).to_le_bytes());
        }
        VirtualRegisterOrigin::EntryParameter {
            source_value,
            parameter_index,
        } => {
            bytes.push(0);
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            bytes.extend_from_slice(&(parameter_index as u64).to_le_bytes());
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
        Some(site) => {
            bytes.push(1);
            encode_definition_site(bytes, site);
        }
        None => bytes.push(0),
    }
    encode_option_u16(bytes, register.entry_fixed_view.map(|view| view.0));
}

fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(0);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(1);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}

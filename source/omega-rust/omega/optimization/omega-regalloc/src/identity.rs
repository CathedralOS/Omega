use omega_register_model::RegisterOperandAccess;
use omega_target::{Architecture, ObjectFormat};
use sha2::{Digest, Sha256};

use crate::model::{TerminalLivenessIdentity, TerminalLivenessPlan};

pub fn terminal_liveness_identity(plan: &TerminalLivenessPlan) -> TerminalLivenessIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-register-liveness.v7\0");
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.target.architecture {
        Architecture::X86_64 => 0,
        Architecture::Aarch64 => 1,
    });
    bytes.push(match plan.target.object_format {
        ObjectFormat::Elf => 0,
        ObjectFormat::MachO => 1,
        ObjectFormat::Coff => 2,
    });
    bytes.extend_from_slice(&(plan.target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(plan.target.pointer_alignment as u64).to_le_bytes());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_len(&mut bytes, function.entry_definitions.len());
        for definition in &function.entry_definitions {
            bytes.extend_from_slice(&definition.virtual_register.0.to_le_bytes());
            bytes.extend_from_slice(&definition.class.0.to_le_bytes());
            encode_option_u16(&mut bytes, definition.fixed_view.map(|view| view.0));
        }
        encode_len(&mut bytes, function.operand_positions.len());
        for operand in &function.operand_positions {
            bytes.extend_from_slice(&operand.position.0.to_le_bytes());
            bytes.extend_from_slice(&operand.instruction.0.to_le_bytes());
            bytes.extend_from_slice(&operand.operand.to_le_bytes());
            bytes.extend_from_slice(&operand.virtual_register.0.to_le_bytes());
            bytes.push(access_tag(operand.access));
            bytes.extend_from_slice(&operand.class.0.to_le_bytes());
            encode_option_u16(&mut bytes, operand.fixed_view.map(|view| view.0));
            encode_option_u16(&mut bytes, operand.tied_to);
            bytes.push(u8::from(operand.early_clobber));
        }
        encode_len(&mut bytes, function.blocks.len());
        for block in &function.blocks {
            bytes.extend_from_slice(&block.block.0.to_le_bytes());
            bytes.extend_from_slice(&block.source_block.get().to_le_bytes());
            encode_vregs(&mut bytes, &block.virtual_live_in);
            encode_vregs(&mut bytes, &block.virtual_live_out);
            encode_units(&mut bytes, &block.unit_live_in);
            encode_units(&mut bytes, &block.unit_live_out);
            encode_len(&mut bytes, block.instructions.len());
            for instruction in &block.instructions {
                bytes.extend_from_slice(&instruction.position.0.to_le_bytes());
                bytes.extend_from_slice(&instruction.instruction.0.to_le_bytes());
                encode_vregs(&mut bytes, &instruction.virtual_uses);
                encode_vregs(&mut bytes, &instruction.virtual_defs);
                encode_vregs(&mut bytes, &instruction.virtual_live_in);
                encode_vregs(&mut bytes, &instruction.virtual_live_out);
                encode_units(&mut bytes, &instruction.unit_uses);
                encode_units(&mut bytes, &instruction.unit_defs);
                encode_units(&mut bytes, &instruction.unit_clobbers);
                encode_units(&mut bytes, &instruction.unit_live_in);
                encode_units(&mut bytes, &instruction.unit_live_out);
            }
            encode_len(&mut bytes, block.successors.len());
            for successor in &block.successors {
                bytes.extend_from_slice(&successor.terminator.0.to_le_bytes());
                bytes.push(successor.polarity_ordinal);
                bytes.extend_from_slice(&successor.psi_edge.get().to_le_bytes());
                bytes.extend_from_slice(&successor.target.0.to_le_bytes());
                encode_vregs(&mut bytes, &successor.virtual_live);
                encode_units(&mut bytes, &successor.unit_live);
            }
        }
    }
    TerminalLivenessIdentity(Sha256::digest(bytes).into())
}

fn access_tag(access: RegisterOperandAccess) -> u8 {
    match access {
        RegisterOperandAccess::Use => 0,
        RegisterOperandAccess::Def => 1,
        RegisterOperandAccess::UseDef => 2,
    }
}

fn encode_option_u16(bytes: &mut Vec<u8>, value: Option<u16>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn encode_vregs(
    bytes: &mut Vec<u8>,
    values: &[omega_terminal_selected_instructions::TerminalVirtualRegisterId],
) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.0.to_le_bytes());
    }
}

fn encode_units(bytes: &mut Vec<u8>, values: &[omega_register_model::RegisterUnitId]) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.0.to_le_bytes());
    }
}

fn encode_len(bytes: &mut Vec<u8>, length: usize) {
    bytes.extend_from_slice(
        &u64::try_from(length)
            .expect("canonical liveness collection length fits u64")
            .to_le_bytes(),
    );
}

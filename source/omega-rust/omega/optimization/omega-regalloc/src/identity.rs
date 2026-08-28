use omega_register_model::RegisterOperandAccess;
use omega_target::{Architecture, ObjectFormat};
use sha2::{Digest, Sha256};

use crate::model::{TerminalFunctionLiveness, TerminalLivenessIdentity, TerminalLivenessPlan};

pub fn terminal_liveness_identity(plan: &TerminalLivenessPlan) -> TerminalLivenessIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-register-liveness.v8\0");
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
        encode_function(&mut bytes, function);
    }
    encode_len(&mut bytes, plan.structural_unit_functions.len());
    for function in &plan.structural_unit_functions {
        encode_function(&mut bytes, function);
    }
    TerminalLivenessIdentity(Sha256::digest(bytes).into())
}

fn encode_function(bytes: &mut Vec<u8>, function: &TerminalFunctionLiveness) {
    bytes.extend_from_slice(&function.machine.get().to_le_bytes());
    encode_len(bytes, function.entry_definitions.len());
    for definition in &function.entry_definitions {
        bytes.extend_from_slice(&definition.virtual_register.0.to_le_bytes());
        bytes.extend_from_slice(&definition.class.0.to_le_bytes());
        encode_option_u16(bytes, definition.fixed_view.map(|view| view.0));
    }
    encode_len(bytes, function.operand_positions.len());
    for operand in &function.operand_positions {
        bytes.extend_from_slice(&operand.position.0.to_le_bytes());
        bytes.extend_from_slice(&operand.instruction.0.to_le_bytes());
        bytes.extend_from_slice(&operand.operand.to_le_bytes());
        bytes.extend_from_slice(&operand.virtual_register.0.to_le_bytes());
        bytes.push(access_tag(operand.access));
        bytes.extend_from_slice(&operand.class.0.to_le_bytes());
        encode_option_u16(bytes, operand.fixed_view.map(|view| view.0));
        encode_option_u16(bytes, operand.tied_to);
        bytes.push(u8::from(operand.early_clobber));
    }
    encode_len(bytes, function.blocks.len());
    for block in &function.blocks {
        bytes.extend_from_slice(&block.block.0.to_le_bytes());
        bytes.extend_from_slice(&block.source_block.get().to_le_bytes());
        encode_vregs(bytes, &block.virtual_live_in);
        encode_vregs(bytes, &block.virtual_live_out);
        encode_units(bytes, &block.unit_live_in);
        encode_units(bytes, &block.unit_live_out);
        encode_len(bytes, block.instructions.len());
        for instruction in &block.instructions {
            bytes.extend_from_slice(&instruction.position.0.to_le_bytes());
            bytes.extend_from_slice(&instruction.instruction.0.to_le_bytes());
            encode_vregs(bytes, &instruction.virtual_uses);
            encode_vregs(bytes, &instruction.virtual_defs);
            encode_vregs(bytes, &instruction.virtual_live_in);
            encode_vregs(bytes, &instruction.virtual_live_out);
            encode_units(bytes, &instruction.unit_uses);
            encode_units(bytes, &instruction.unit_defs);
            encode_units(bytes, &instruction.unit_clobbers);
            encode_units(bytes, &instruction.unit_live_in);
            encode_units(bytes, &instruction.unit_live_out);
        }
        encode_len(bytes, block.successors.len());
        for successor in &block.successors {
            bytes.extend_from_slice(&successor.terminator.0.to_le_bytes());
            bytes.push(successor.polarity_ordinal);
            bytes.extend_from_slice(&successor.psi_edge.get().to_le_bytes());
            bytes.extend_from_slice(&successor.target.0.to_le_bytes());
            encode_vregs(bytes, &successor.virtual_live);
            encode_units(bytes, &successor.unit_live);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        TerminalBlockLiveness, TerminalFunctionLiveness, TerminalInstructionLiveness,
        TerminalLivenessPosition,
    };
    use omega_optimization_core::OptimizationUnitIdentity;
    use omega_register_model::RegisterUnitId;
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlockId, TerminalSelectedInstructionId,
        TerminalSelectedInstructionPlanIdentity,
    };
    use psi_core::{BlockId, FuelScheduleIdentity, MachineId};

    fn function(machine: u64, unit: u16) -> TerminalFunctionLiveness {
        TerminalFunctionLiveness {
            machine: MachineId::new(machine).unwrap(),
            entry_definitions: Vec::new(),
            operand_positions: Vec::new(),
            blocks: vec![TerminalBlockLiveness {
                block: TerminalSelectedBlockId(0),
                source_block: BlockId::new(machine).unwrap(),
                virtual_live_in: Vec::new(),
                virtual_live_out: Vec::new(),
                unit_live_in: vec![RegisterUnitId(unit)],
                unit_live_out: Vec::new(),
                instructions: vec![TerminalInstructionLiveness {
                    position: TerminalLivenessPosition(0),
                    instruction: TerminalSelectedInstructionId(0),
                    virtual_uses: Vec::new(),
                    virtual_defs: Vec::new(),
                    virtual_live_in: Vec::new(),
                    virtual_live_out: Vec::new(),
                    unit_uses: vec![RegisterUnitId(unit)],
                    unit_defs: Vec::new(),
                    unit_clobbers: Vec::new(),
                    unit_live_in: vec![RegisterUnitId(unit)],
                    unit_live_out: Vec::new(),
                }],
                successors: Vec::new(),
            }],
        }
    }

    fn plan() -> TerminalLivenessPlan {
        TerminalLivenessPlan {
            selected: TerminalSelectedInstructionPlanIdentity::from_bytes([1; 32]),
            optimization_unit: OptimizationUnitIdentity::from_canonical_bytes(b"unit"),
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target: omega_target::NativeTarget::linux_x64(),
            functions: Vec::new(),
            structural_unit_functions: vec![function(1, 1), function(2, 2)],
        }
    }

    #[test]
    fn v8_identity_binds_structural_roster_order_machine_instruction_and_units() {
        let plan = plan();
        let identity = terminal_liveness_identity(&plan);
        assert_eq!(identity, terminal_liveness_identity(&plan));

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions.pop();
        assert_ne!(identity, terminal_liveness_identity(&corrupted));

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions.swap(0, 1);
        assert_ne!(identity, terminal_liveness_identity(&corrupted));

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0].machine = MachineId::new(3).unwrap();
        assert_ne!(identity, terminal_liveness_identity(&corrupted));

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0].blocks[0].instructions[0].instruction =
            TerminalSelectedInstructionId(1);
        assert_ne!(identity, terminal_liveness_identity(&corrupted));

        let mut corrupted = plan.clone();
        corrupted.structural_unit_functions[0].blocks[0].instructions[0].unit_uses[0] =
            RegisterUnitId(3);
        assert_ne!(identity, terminal_liveness_identity(&corrupted));
    }
}

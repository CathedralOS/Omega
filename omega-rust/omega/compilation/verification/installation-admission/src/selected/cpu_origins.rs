//! Capture source-only CPU queries while physical planning owns the instructions.
//! The byte receiver keeps its independent frame, branch and ISA-effect decoding.
use machine_code::cpu_origins::*;
use selected_instructions::{RegisterOperation, SelectedInstructionKind};

pub fn capture(
    program: &post_allocation_machine::PostAllocationMachineProgram,
) -> PhysicalCpuOrigins {
    PhysicalCpuOrigins {
        instruction_roster: program
            .machine
            .functions
            .iter()
            .flat_map(|function| {
                function
                    .instructions()
                    .map(move |row| (function.machine, row.instruction))
            })
            .collect(),
        functions: program
            .functions
            .iter()
            .map(|function| {
                let mut result = PhysicalFunctionCpuOrigins {
                    machine: function.machine,
                    unmasks_interrupts: Vec::new(),
                    indirect_calls: Vec::new(),
                    model_specific_registers: Vec::new(),
                };
                for instruction in function.instructions() {
                    if let Some(target) = instruction
                        .kind
                        .call_target()
                        .filter(|target| target.machine().is_none())
                    {
                        result.indirect_calls.push((instruction.id, target));
                    }
                    let SelectedInstructionKind::RegisterAccess { operation } = instruction.kind
                    else {
                        continue;
                    };
                    let access = match operation {
                        RegisterOperation::InterruptControl(
                            terminal_psi::InterruptControlKind::Enable,
                        ) => {
                            result
                                .unmasks_interrupts
                                .push(InterruptMaskChange::Enable(instruction.id));
                            continue;
                        }
                        RegisterOperation::SystemWrite(terminal_psi::SystemRegister::Rflags) => {
                            result
                                .unmasks_interrupts
                                .push(InterruptMaskChange::RestoreUnknownFlags(instruction.id));
                            continue;
                        }
                        RegisterOperation::ModelSpecificRead => ModelSpecificAccess::Read,
                        RegisterOperation::ModelSpecificWrite => ModelSpecificAccess::Write,
                        _ => continue,
                    };
                    result
                        .model_specific_registers
                        .push(ModelSpecificRegisterSource {
                            instruction: instruction.id,
                            access,
                            selector: super::selected_constant_operand(function, instruction.id, 0)
                                .map_err(|error| error.to_string()),
                        });
                }
                result
            })
            .collect(),
    }
}

use crate::{
    MachineInstruction, MachineInstructionCode, MachineInstructionFunction, MachineInstructionPlan,
};
use omega_assigned_target_operations::SelectedInstructionKind;
use omega_core::arena::Arena;
use std::convert::Infallible;

impl From<omega_machine_program::MachineProgram> for MachineInstructionPlan {
    fn from(program: omega_machine_program::MachineProgram) -> Self {
        let mut code = MachineInstructionCode {
            functions: Arena::with_capacity(program.code.functions.len()),
            instructions: Arena::with_capacity(program.code.instructions.len()),
        };

        for (_, function) in program.code.functions.iter() {
            let Some(function_instructions) = program.code.instructions.span(function.instructions)
            else {
                continue;
            };
            let inserted = code
                .instructions
                .try_insert_many(function_instructions.iter().map(|instruction| {
                    Ok::<MachineInstruction, Infallible>(MachineInstruction {
                        selected_instruction_index: instruction.selected_instruction_index,
                        source_kind: SelectedInstructionKind::EnterFunction,
                        kind: instruction.kind,
                    })
                }))
                .expect("machine instruction arena insertion should not fail");
            code.functions.insert(MachineInstructionFunction {
                source_key: function.source_key,
                instructions: inserted,
            });
        }

        Self::with_roots(program.target, code, program.semantics)
    }
}

impl From<MachineInstructionPlan> for omega_machine_program::MachineProgram {
    fn from(plan: MachineInstructionPlan) -> Self {
        let mut code = omega_machine_program::MachineProgramCode {
            functions: Arena::with_capacity(plan.code.functions.len()),
            instructions: Arena::with_capacity(plan.code.instructions.len()),
        };

        for (_, function) in plan.code.functions.iter() {
            let Some(function_instructions) = plan.code.instructions.span(function.instructions)
            else {
                continue;
            };
            let inserted = code
                .instructions
                .try_insert_many(function_instructions.iter().map(|instruction| {
                    Ok::<omega_machine_program::MachineInstruction, Infallible>(
                        omega_machine_program::MachineInstruction {
                            selected_instruction_index: instruction.selected_instruction_index,
                            kind: instruction.kind,
                        },
                    )
                }))
                .expect("machine instruction arena insertion should not fail");
            code.functions
                .insert(omega_machine_program::MachineFunction {
                    source_key: function.source_key,
                    instructions: inserted,
                });
        }

        omega_machine_program::MachineProgram::with_roots(plan.target, code, plan.semantics)
    }
}

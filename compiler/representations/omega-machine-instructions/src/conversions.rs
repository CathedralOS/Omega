use crate::{MachineInstruction, MachineInstructionFunction, MachineInstructionPlan};
use omega_assigned_target_operations::SelectedInstructionKind;
use std::convert::Infallible;

impl From<omega_machine_program::MachineProgram> for MachineInstructionPlan {
    fn from(program: omega_machine_program::MachineProgram) -> Self {
        let mut plan = Self::with_capacity(
            program.target,
            program.code.functions.len(),
            program.code.instructions.len(),
        );
        for (_, function) in program.code.functions.iter() {
            let Some(function_instructions) = program.code.instructions.span(function.instructions)
            else {
                continue;
            };
            let inserted = plan
                .code
                .instructions
                .try_insert_many(function_instructions.iter().map(|instruction| {
                    Ok::<MachineInstruction, Infallible>(MachineInstruction {
                        selected_instruction_index: instruction.selected_instruction_index,
                        source_kind: SelectedInstructionKind::EnterFunction,
                        kind: instruction.kind,
                    })
                }))
                .expect("machine instruction arena insertion should not fail");
            plan.code.functions.insert(MachineInstructionFunction {
                source_key: function.source_key,
                instructions: inserted,
            });
        }
        plan.semantics.values = program.semantics.values;
        plan.semantics.boundary_edges = program.semantics.boundary_edges;
        plan.semantics.ownership = program.semantics.ownership;
        plan
    }
}

impl From<MachineInstructionPlan> for omega_machine_program::MachineProgram {
    fn from(plan: MachineInstructionPlan) -> Self {
        let mut program = omega_machine_program::MachineProgram::with_capacity(
            plan.target,
            plan.code.functions.len(),
            plan.code.instructions.len(),
        );
        for (_, function) in plan.code.functions.iter() {
            let Some(function_instructions) = plan.code.instructions.span(function.instructions)
            else {
                continue;
            };
            let inserted = program
                .code
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
            program
                .code
                .functions
                .insert(omega_machine_program::MachineFunction {
                    source_key: function.source_key,
                    instructions: inserted,
                });
        }
        program.semantics.values = plan.semantics.values;
        program.semantics.boundary_edges = plan.semantics.boundary_edges;
        program.semantics.ownership = plan.semantics.ownership;
        program
    }
}

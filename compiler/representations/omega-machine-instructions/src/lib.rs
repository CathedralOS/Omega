use omega_assigned_target_operations::SelectedInstructionKind;
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_target::NativeTarget;
use std::convert::Infallible;

pub use omega_machine_program::MachineInstructionKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineInstructionPlan {
    pub target: NativeTarget,
    pub functions: Arena<MachineInstructionFunction>,
    pub instructions: Arena<MachineInstruction>,
}

impl Default for MachineInstructionPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0)
    }
}

impl MachineInstructionPlan {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
    ) -> Self {
        Self {
            target,
            functions: Arena::with_capacity(function_capacity),
            instructions: Arena::with_capacity(instruction_capacity),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineInstructionFunction {
    pub source_key: StateKey,
    pub instructions: HandleSpan<MachineInstruction>,
}

impl Default for MachineInstructionFunction {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            instructions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineInstruction {
    pub selected_instruction_index: u32,
    pub source_kind: SelectedInstructionKind,
    pub kind: MachineInstructionKind,
}

impl Default for MachineInstruction {
    fn default() -> Self {
        Self {
            selected_instruction_index: 0,
            source_kind: SelectedInstructionKind::EnterFunction,
            kind: MachineInstructionKind::NoOp,
        }
    }
}

impl From<omega_machine_program::MachineProgram> for MachineInstructionPlan {
    fn from(program: omega_machine_program::MachineProgram) -> Self {
        let mut plan = Self::with_capacity(
            program.target,
            program.functions.len(),
            program.instructions.len(),
        );
        for (_, function) in program.functions.iter() {
            let Some(function_instructions) = program.instructions.span(function.instructions) else {
                continue;
            };
            let inserted = plan.instructions.try_insert_many(function_instructions.iter().map(
                |instruction| Ok::<MachineInstruction, Infallible>(MachineInstruction {
                        selected_instruction_index: instruction.selected_instruction_index,
                        source_kind: SelectedInstructionKind::EnterFunction,
                        kind: instruction.kind,
                    }),
            )).expect("machine instruction arena insertion should not fail");
            plan.functions.insert(MachineInstructionFunction {
                source_key: function.source_key,
                instructions: inserted,
            });
        }
        plan
    }
}

impl From<MachineInstructionPlan> for omega_machine_program::MachineProgram {
    fn from(plan: MachineInstructionPlan) -> Self {
        let mut program = omega_machine_program::MachineProgram::with_capacity(
            plan.target,
            plan.functions.len(),
            plan.instructions.len(),
        );
        for (_, function) in plan.functions.iter() {
            let Some(function_instructions) = plan.instructions.span(function.instructions) else {
                continue;
            };
            let inserted = program.instructions.try_insert_many(function_instructions.iter().map(
                |instruction| Ok::<omega_machine_program::MachineInstruction, Infallible>(omega_machine_program::MachineInstruction {
                        selected_instruction_index: instruction.selected_instruction_index,
                        kind: instruction.kind,
                    }),
            )).expect("machine instruction arena insertion should not fail");
            program.functions.insert(omega_machine_program::MachineFunction {
                source_key: function.source_key,
                instructions: inserted,
            });
        }
        program
    }
}

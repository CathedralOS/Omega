use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_target::NativeTarget;

pub use omega_machine_program::{MachineInstruction, MachineInstructionKind};

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

impl From<omega_machine_program::MachineProgram> for MachineInstructionPlan {
    fn from(program: omega_machine_program::MachineProgram) -> Self {
        let mut functions = Arena::with_capacity(program.functions.len());
        for (_, function) in program.functions.iter() {
            functions.insert(MachineInstructionFunction {
                source_key: function.source_key,
                instructions: function.instructions,
            });
        }

        Self {
            target: program.target,
            functions,
            instructions: program.instructions,
        }
    }
}

impl From<MachineInstructionPlan> for omega_machine_program::MachineProgram {
    fn from(plan: MachineInstructionPlan) -> Self {
        let mut functions = Arena::with_capacity(plan.functions.len());
        for (_, function) in plan.functions.iter() {
            functions.insert(omega_machine_program::MachineFunction {
                source_key: function.source_key,
                instructions: function.instructions,
            });
        }

        Self {
            target: plan.target,
            functions,
            instructions: plan.instructions,
        }
    }
}

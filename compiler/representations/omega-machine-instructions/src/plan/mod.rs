mod capacity;
mod code;

pub use code::{MachineInstructionCode, MachineInstructionPlan};

impl Default for MachineInstructionPlan {
    fn default() -> Self {
        Self::with_capacity(omega_target::NativeTarget::host(), 0, 0)
    }
}

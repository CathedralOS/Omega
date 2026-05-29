mod capacity;
mod code;

pub use code::{MachineProgram, MachineProgramCode};

impl Default for MachineProgram {
    fn default() -> Self {
        Self::with_capacity(omega_target::NativeTarget::host(), 0, 0)
    }
}

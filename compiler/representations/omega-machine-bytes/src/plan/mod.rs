mod capacity;
mod code;

pub use code::{EncodedMachineCode, EncodedMachinePlan};

impl Default for EncodedMachinePlan {
    fn default() -> Self {
        Self::with_capacity(omega_target::NativeTarget::host(), 0, 0, 0)
    }
}

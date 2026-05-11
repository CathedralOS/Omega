use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedMachinePlan {
    pub target: NativeTarget,
    pub functions: Arena<EncodedMachineFunction>,
    pub instructions: Arena<EncodedMachineInstruction>,
    pub bytes: Arena<u8>,
    pub byte_count: usize,
}

impl Default for EncodedMachinePlan {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            functions: Arena::new(),
            instructions: Arena::new(),
            bytes: Arena::new(),
            byte_count: 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EncodedMachineFunction {
    pub symbol: String,
    pub source_key: StateKey,
    pub byte_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EncodedMachineInstruction {
    pub selected_instruction_index: u32,
    pub bytes: HandleSpan<u8>,
}

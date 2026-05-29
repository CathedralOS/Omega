use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_target::NativeTarget;

pub type EncodedMachineBoundarySummary = omega_target_operations::TargetBoundarySummary;
pub type EncodedMachineOwnershipSummary = omega_target_operations::TargetOwnershipSummary;
pub type EncodedMachineValueSummary = omega_target_operations::TargetValueSummary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedMachinePlan {
    pub target: NativeTarget,
    pub functions: Arena<EncodedMachineFunction>,
    pub instructions: Arena<EncodedMachineInstruction>,
    pub bytes: Arena<u8>,
    pub byte_count: usize,
    pub values: EncodedMachineValueSummary,
    pub boundary_edges: EncodedMachineBoundarySummary,
    pub ownership: EncodedMachineOwnershipSummary,
}

impl Default for EncodedMachinePlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0)
    }
}

impl EncodedMachinePlan {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
        byte_capacity: usize,
    ) -> Self {
        Self {
            target,
            functions: Arena::with_capacity(function_capacity),
            instructions: Arena::with_capacity(instruction_capacity),
            bytes: Arena::with_capacity(byte_capacity),
            byte_count: 0,
            values: EncodedMachineValueSummary::default(),
            boundary_edges: EncodedMachineBoundarySummary::default(),
            ownership: EncodedMachineOwnershipSummary::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedMachineFunction {
    pub source_key: StateKey,
    pub byte_offset: usize,
    pub byte_count: usize,
}

impl Default for EncodedMachineFunction {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            byte_offset: 0,
            byte_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EncodedMachineInstruction {
    pub selected_instruction_index: u32,
    pub bytes: HandleSpan<u8>,
}

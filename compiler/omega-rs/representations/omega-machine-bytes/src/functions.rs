use omega_control_flow::StateKey;
use psi_arena::HandleSpan;
use std::sync::Arc;

use crate::EncodedMachineInstruction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedMachineFunction {
    pub symbol: Arc<str>,
    pub source_key: StateKey,
    pub byte_offset: usize,
    pub byte_count: usize,
    /// Exact contiguous encoded-instruction rows owned by this function.
    /// Final-image validation uses this retained boundary instead of scanning
    /// arbitrary text for instruction starts.
    pub instructions: HandleSpan<EncodedMachineInstruction>,
}

impl Default for EncodedMachineFunction {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            source_key: StateKey::default(),
            byte_offset: 0,
            byte_count: 0,
            instructions: HandleSpan::empty(),
        }
    }
}

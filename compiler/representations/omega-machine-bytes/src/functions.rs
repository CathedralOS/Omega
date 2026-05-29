use omega_control_flow::StateKey;

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

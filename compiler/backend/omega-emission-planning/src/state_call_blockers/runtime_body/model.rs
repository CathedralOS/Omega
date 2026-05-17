use omega_control_flow::StateKey;
use omega_state_calls::StateCallLowering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeBodyStateCallBlocker {
    pub(super) dispatch_index: u32,
    pub(super) source_key: StateKey,
    pub(super) first_statement_index: usize,
    pub(super) target_key: StateKey,
    pub(super) argument_count: usize,
    pub(super) lowering: StateCallLowering,
    pub(super) count: usize,
}

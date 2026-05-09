use crate::state_calls::StateCallLowering;
use omega_control_flow::StateKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeBodyStateCallBlocker {
    pub(super) dispatch_index: u32,
    pub(super) source_key: StateKey,
    pub(super) source_machine: String,
    pub(super) source_state: String,
    pub(super) first_statement_index: usize,
    pub(super) target_key: StateKey,
    pub(super) target_machine: String,
    pub(super) target_state: String,
    pub(super) argument_count: usize,
    pub(super) lowering: StateCallLowering,
    pub(super) count: usize,
}

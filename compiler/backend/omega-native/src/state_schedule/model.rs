use omega_control_flow::StateKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledState {
    pub key: StateKey,
}

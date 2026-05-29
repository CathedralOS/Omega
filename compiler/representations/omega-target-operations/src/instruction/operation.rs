use super::TargetOperationKind;
use omega_control_flow::StateKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperation {
    pub kind: TargetOperationKind,
    pub source_key: StateKey,
    pub source_statement: usize,
}

pub type SelectedInstruction = TargetOperation;

impl Default for TargetOperation {
    fn default() -> Self {
        Self {
            kind: TargetOperationKind::EnterFunction,
            source_key: StateKey::default(),
            source_statement: 0,
        }
    }
}

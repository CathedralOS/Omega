use super::AbstractOperationKind;
use omega_control_flow::StateKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperation {
    pub kind: AbstractOperationKind,
    pub source_key: StateKey,
    pub source_statement: usize,
}

pub type SelectedInstruction = AbstractOperation;

impl Default for AbstractOperation {
    fn default() -> Self {
        Self {
            kind: AbstractOperationKind::EnterFunction,
            source_key: StateKey::default(),
            source_statement: 0,
        }
    }
}

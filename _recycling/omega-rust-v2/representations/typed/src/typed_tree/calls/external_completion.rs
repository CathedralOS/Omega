use crate::Site;
use omega_resolved::{DeclarationId, TreeNodeId};

/// The closed set of non-crashing external terminal outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalCompletionKind {
    /// The canonical `ProcessExit::exit_process` requirement ends the bound process domain.
    ProcessExit,
}

/// An execution call L3 recognized as the exact admitted canonical requirement,
/// including its package and owning boundary trait. A matching path or method
/// spelling alone never produces this record. Recognition grants no authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalCompletion {
    pub node: TreeNodeId,
    pub site: Site,
    pub kind: ExternalCompletionKind,
    pub owner: DeclarationId,
    pub requirement: DeclarationId,
}

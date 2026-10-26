//! Terminal identities and exact call-site ownership.

use semantic_vocabulary::{EdgeId, InvocationSite, OperationId};

/// Ordered terminal-Psi sources refined into one target function.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalPsiProvenance {
    pub operations: Vec<OperationId>,
    pub edges: Vec<EdgeId>,
}

/// Semantic owner of one native call site.
///
/// Ordinary calls are owned by terminal-Psi operations. An executable nominal
/// cleanup is different: invoking the attached cleanup machine is work of one
/// exact ordered action on the selected ownership edge, so it must retain both
/// identities rather than fabricating an [`OperationId`]. An admitted external
/// completion owns its exact terminal edge and no normal result or successor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallSiteOwner {
    Operation(OperationId),
    ExternalCompletion(EdgeId),
    CleanupAction { edge: EdgeId, action_ordinal: u32 },
}

impl CallSiteOwner {
    pub const fn operation(self) -> Option<OperationId> {
        match self {
            Self::Operation(operation) => Some(operation),
            Self::ExternalCompletion(_) | Self::CleanupAction { .. } => None,
        }
    }

    pub const fn edge(self) -> Option<EdgeId> {
        match self {
            Self::Operation(_) => None,
            Self::ExternalCompletion(edge) | Self::CleanupAction { edge, .. } => Some(edge),
        }
    }

    pub const fn cleanup_action_ordinal(self) -> Option<u32> {
        match self {
            Self::Operation(_) | Self::ExternalCompletion(_) => None,
            Self::CleanupAction { action_ordinal, .. } => Some(action_ordinal),
        }
    }
}

impl From<InvocationSite> for CallSiteOwner {
    fn from(site: InvocationSite) -> Self {
        match site {
            InvocationSite::Operation(operation) => Self::Operation(operation),
            InvocationSite::ExternalCompletion(edge) => Self::ExternalCompletion(edge),
        }
    }
}

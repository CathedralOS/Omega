// Terminal Psi provenance, call-site ownership, and admitted boundary origin facts.
// Ordinary calls belong to Terminal operations, nominal cleanup calls retain their ownership edge and action ordinal, external completions own their terminal edge, and static-interrupt installation helpers keep the source operation alongside their distinct `Validate` or `Work` role.

use installation_evidence::InstalledAssumedBindingEvidence;
use semantic_vocabulary::{EdgeId, InvocationSite, OperationId};
use terminal_installation::AdmittedTargetMachineSelections;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalPsiProvenance {
    pub operations: Vec<OperationId>, // Ordered Terminal Psi operations refined into one target function.
    pub edges: Vec<EdgeId>, // Ordered Terminal Psi control edges refined into one target function.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InstallationCallRole {
    Validate,
    Work,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallSiteOwner {
    Operation(OperationId),
    InvokeNoNormal(EdgeId),
    ExternalCompletion(EdgeId),
    CleanupAction {
        edge: EdgeId,
        action_ordinal: u32,
    },
    Installation {
        operation: OperationId,
        role: InstallationCallRole,
    },
}

impl CallSiteOwner {
    pub const fn operation(self) -> Option<OperationId> {
        match self {
            Self::Operation(operation) | Self::Installation { operation, .. } => Some(operation),
            Self::InvokeNoNormal(_) | Self::ExternalCompletion(_) | Self::CleanupAction { .. } => {
                None
            }
        }
    }

    pub const fn edge(self) -> Option<EdgeId> {
        match self {
            Self::Operation(_) | Self::Installation { .. } => None,
            Self::InvokeNoNormal(edge)
            | Self::ExternalCompletion(edge)
            | Self::CleanupAction { edge, .. } => Some(edge),
        }
    }

    pub const fn cleanup_action_ordinal(self) -> Option<u32> {
        match self {
            Self::Operation(_)
            | Self::Installation { .. }
            | Self::InvokeNoNormal(_)
            | Self::ExternalCompletion(_) => None,
            Self::CleanupAction { action_ordinal, .. } => Some(action_ordinal),
        }
    }
}

impl From<InvocationSite> for CallSiteOwner {
    fn from(site: InvocationSite) -> Self {
        match site {
            InvocationSite::InvokeNoNormal(edge) => Self::InvokeNoNormal(edge),
            InvocationSite::Operation(operation) => Self::Operation(operation),
            InvocationSite::ExternalCompletion(edge) => Self::ExternalCompletion(edge),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BoundaryOriginFacts<'input> {
    pub program_entry: Option<ProgramEntryOrigin>,
    pub assumed_bindings: &'input [InstalledAssumedBindingEvidence],
    pub target_machine_selections: Option<&'input AdmittedTargetMachineSelections>,
    pub module: &'input terminal_psi::TerminalModule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramEntryOrigin {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub entry: semantic_vocabulary::MachineId,
    pub source_signature: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::{CallSiteOwner, InstallationCallRole};
    use semantic_vocabulary::{EdgeId, OperationId};

    #[test]
    fn call_site_owner_preserves_disjoint_operation_and_cleanup_action_identities() {
        let operation = OperationId::new(7).expect("nonzero operation");
        let edge = EdgeId::new(7).expect("nonzero edge");

        let operation_owner = CallSiteOwner::Operation(operation);
        assert_eq!(operation_owner.operation(), Some(operation));
        assert_eq!(operation_owner.edge(), None);

        let edge_owner = CallSiteOwner::CleanupAction {
            edge,
            action_ordinal: 3,
        };
        assert_eq!(edge_owner.operation(), None);
        assert_eq!(edge_owner.edge(), Some(edge));
        assert_eq!(edge_owner.cleanup_action_ordinal(), Some(3));
        assert_eq!(operation_owner.cleanup_action_ordinal(), None);
        assert_ne!(operation_owner, edge_owner);
    }

    #[test]
    fn installation_helpers_keep_source_operation_and_distinct_roles() {
        let operation = OperationId::new(7).expect("operation");
        let validate = CallSiteOwner::Installation {
            operation,
            role: InstallationCallRole::Validate,
        };
        let work = CallSiteOwner::Installation {
            operation,
            role: InstallationCallRole::Work,
        };
        for helper in [validate, work] {
            assert_eq!(helper.operation(), Some(operation));
            assert_eq!(helper.edge(), None);
            assert_eq!(helper.cleanup_action_ordinal(), None);
            assert_ne!(helper, CallSiteOwner::Operation(operation));
        }
        assert_ne!(validate, work);
    }
}

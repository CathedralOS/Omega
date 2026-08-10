use psi_arena::{Arena, HandleSpan};
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatePermissionEvent {
    pub source: psi_language_semantics::PermissionEventSource,
    pub kind: psi_language_semantics::PermissionEventKind,
    pub multiplicity: psi_language_semantics::Multiplicity,
    pub access: psi_language_semantics::PermissionAccess,
    pub claim_identity: psi_language_semantics::PermissionClaimIdentity,
    pub provenance: psi_language_semantics::PermissionProvenance,
    pub root: psi_facts::PlaceRoot,
    pub segments: HandleSpan<psi_facts::PlaceSegment>,
    pub obligation_live: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateOwnershipSummary {
    pub permissions: HandleSpan<StatePermissionEvent>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphOwnershipRoots {
    pub segments: Arena<psi_facts::PlaceSegment>,
    pub permissions: Arena<StatePermissionEvent>,
}

impl StateGraphOwnershipRoots {
    pub fn with_roots(
        segments: Arena<psi_facts::PlaceSegment>,
        permissions: Arena<StatePermissionEvent>,
    ) -> Self {
        Self {
            segments,
            permissions,
        }
    }
}

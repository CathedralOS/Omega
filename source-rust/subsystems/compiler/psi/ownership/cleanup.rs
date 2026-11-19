use crate::{
    ClaimTransfer, NominalDeclarationIdentity, StructuralArgument, StructuralPathSegment,
};
use semantic_vocabulary::{MachineId, ObligationId, PlaceId, StructuralTypeId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralAffineDiscard {
    pub place: PlaceId,
    pub path: Vec<StructuralPathSegment>,
    pub structural_type: StructuralTypeId,
}

/// The carrier's declaration selects an exact ordinary cleanup machine.
/// Keep the original declaration separate from a closed type application:
/// instantiation does not change who may end the nominal wrapper.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NominalCleanupSelection {
    pub declaration: NominalDeclarationIdentity,
    pub cleanup_machine: MachineId,
}

/// A death edge transfers its subject to an ordinary owned parameter.
/// Component and independent claims follow the same correspondence as a
/// source call; invoking cleanup does not itself settle those claims.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnedCleanupInvocation {
    pub subject: StructuralArgument,
    pub structural_type: StructuralTypeId,
    pub cleanup_machine: MachineId,
    pub claim_transfers: Vec<ClaimTransfer>,
    pub requirement_obligations: Vec<ObligationId>,
}

impl OwnedCleanupInvocation {
    pub fn module_invocation(&self) -> crate::ModuleInvocationInputs<'_> {
        crate::ModuleInvocationInputs {
            callee: self.cleanup_machine,
            required_contract: None,
            arguments: &[],
            erased_arguments: &[],
            erased_proof_arguments: &[],
            structural_arguments: std::slice::from_ref(&self.subject),
            claim_transfers: &self.claim_transfers,
            completion_receipts: &[],
            requirement_obligations: &self.requirement_obligations,
            crash_continuations: &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalAffineCleanupAction {
    DiscardRoot(PlaceId),
    DiscardResidual(StructuralAffineDiscard),
    InvokeOwned(OwnedCleanupInvocation),
    RetireHome(PlaceId),
}

impl TerminalAffineCleanupAction {
    pub fn path(&self) -> &[StructuralPathSegment] {
        match self {
            Self::DiscardRoot(_) | Self::RetireHome(_) => &[],
            Self::DiscardResidual(discard) => &discard.path,
            Self::InvokeOwned(cleanup) => &cleanup.subject.path,
        }
    }

    pub fn invoked_machine(&self) -> Option<MachineId> {
        match self {
            Self::InvokeOwned(cleanup) => Some(cleanup.cleanup_machine),
            _ => None,
        }
    }

    pub fn place(&self) -> PlaceId {
        match self {
            Self::DiscardRoot(place) | Self::RetireHome(place) => *place,
            Self::DiscardResidual(discard) => discard.place,
            Self::InvokeOwned(cleanup) => cleanup.subject.place,
        }
    }
}

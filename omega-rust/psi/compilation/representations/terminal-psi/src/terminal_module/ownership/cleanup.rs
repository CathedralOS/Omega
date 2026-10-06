use crate::StructuralPathSegment;
use semantic_vocabulary::{ClaimId, MachineId, ObligationId, PlaceId, StructuralTypeId};

/// One exact claim-free affine structural place disposed on an ordinary edge.
/// Unlike the root-only trivial discard vocabulary, this action retains the
/// canonical path and independently checkable leaf type reached by that path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralAffineDiscard {
    pub place: PlaceId,
    pub path: Vec<StructuralPathSegment>,
    pub structural_type: StructuralTypeId,
}

/// One intact owned subtree disposed by its owner-designated cleanup machine.
/// The edge begins consumption before lending the intact value to that hook.
/// Linear disposition retains its complete claim roster; affine cleanup has none.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NominalAffineCleanup {
    pub place: PlaceId,
    /// Canonical containment path from the owner root to the intact receiver.
    /// An empty path selects the whole root; siblings retain their own custody.
    pub path: Vec<StructuralPathSegment>,
    pub structural_type: StructuralTypeId,
    pub cleanup_machine: MachineId,
    /// Strictly ordered machine-local identities consumed by this action.
    /// Independent frontier replay requires exactly the live whole-root claims,
    /// and the target's owner designation supplies the disposition authority.
    pub consumed_claims: Vec<ClaimId>,
    /// The place the edge lends the consumed value to inside the callee
    /// frame: exactly the hook's declared `is_self` structural parameter when
    /// the hook keeps a borrowed `self` receiver, otherwise a proof-local
    /// root any contextual `requires` clause roots at. Absent when the hook
    /// keeps no receiver and declares no contextual requirements. It is not
    /// an argument slot on the edge itself.
    pub cleanup_receiver: Option<PlaceId>,
    /// Obligation identities aligned positionally with the selected cleanup
    /// machine's contextual `requires` clauses.
    pub requirement_obligations: Vec<ObligationId>,
}

/// One exact affine cleanup action committed by a terminal ownership edge.
/// The surrounding vector is the semantic execution order; consumers must not
/// regroup actions by kind or reconstruct their order from declarations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalAffineCleanupAction {
    DiscardRoot(PlaceId),
    DiscardResidual(StructuralAffineDiscard),
    InvokeNominal(NominalAffineCleanup),
}

impl TerminalAffineCleanupAction {
    /// The owned root affected by this ordered cleanup action.
    pub fn place(&self) -> PlaceId {
        match self {
            Self::DiscardRoot(place) => *place,
            Self::DiscardResidual(discard) => discard.place,
            Self::InvokeNominal(cleanup) => cleanup.place,
        }
    }
}

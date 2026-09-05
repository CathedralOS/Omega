use crate::StructuralPathSegment;
use semantic_vocabulary::{MachineId, ObligationId, PlaceId, StructuralTypeId};

/// One exact claim-free affine structural place disposed on an ordinary edge.
/// Unlike the root-only trivial discard vocabulary, this action retains the
/// canonical path and independently checkable leaf type reached by that path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralAffineDiscard {
    pub place: PlaceId,
    pub path: Vec<StructuralPathSegment>,
    pub structural_type: StructuralTypeId,
}

/// One whole claim-free affine structural parameter disposed by its exact
/// nominal cleanup machine. Unlike a trivial affine discard, this action is
/// executable edge work and therefore retains the selected machine identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NominalAffineCleanup {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub cleanup_machine: MachineId,
    /// Target-contract-local proof root for the borrowed cleanup receiver.
    /// This is not an executable structural parameter or ABI argument.
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

use crate::StructuralPathSegment;
use semantic_vocabulary::{ClaimId, MachineId, ObligationId, PlaceId, StructuralTypeId};

/// One exact claim-free affine structural place disposed on an ordinary edge.
/// Unlike the intrinsically retiring root discard, this action retains the
/// home binding. Its canonical path and exact leaf type select the disposed
/// region; an empty path selects the whole value, not home retirement.
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
    /// The hook borrows its intact subject while the checked descendant
    /// disposal partition remains pending, possibly across private edges.
    /// A leaf invocation consumes its region; a retaining invocation does not.
    pub retains_descendants: bool,
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
    /// Retire the selected dying owner's frame home after its complete disposal
    /// partition. This is not a subtree discard or a loan restoration receipt;
    /// backing retained by captured outgoing values is independent of the home.
    RetireHome(PlaceId),
}

impl TerminalAffineCleanupAction {
    /// Exact selected region. Whole discard and home retirement both return
    /// the empty path, but retirement disposes no region; consumers deciding
    /// disposal authority must retain the action's kind as well as this path.
    pub fn path(&self) -> &[StructuralPathSegment] {
        match self {
            Self::DiscardRoot(_) | Self::RetireHome(_) => &[],
            Self::DiscardResidual(discard) => &discard.path,
            Self::InvokeNominal(cleanup) => &cleanup.path,
        }
    }

    /// The cleanup machine a nominal invoke delegates to.
    pub fn invoked_machine(&self) -> Option<MachineId> {
        match self {
            Self::InvokeNominal(cleanup) => Some(cleanup.cleanup_machine),
            _ => None,
        }
    }

    /// The owned root affected by this ordered cleanup action.
    pub fn place(&self) -> PlaceId {
        match self {
            Self::DiscardRoot(place) | Self::RetireHome(place) => *place,
            Self::DiscardResidual(discard) => discard.place,
            Self::InvokeNominal(cleanup) => cleanup.place,
        }
    }
}

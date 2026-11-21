//! An ordinary invocation can park before its result enters the caller.
use crate::{BlockId, CallSite, CallTarget, OperationId, PauseEffects, TypeId, ValueId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallAcknowledgement {
    pub call: CallSite,
    pub effects: PauseEffects,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionContinuation {
    Operation(OperationId),
    Terminator(BlockId),
    Return,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivationValue {
    pub value: ValueId,
    pub ty: TypeId,
    pub carry: crate::Carry,
    pub needed_by_callee: bool,
    pub needed_after_completion: bool,
}

/// A loan descriptor held by an incomplete call or its caller’s continuation.
/// The descriptor names the value live at the crossing. Follow a block
/// parameter through the arriving edge, or a completed call through its
/// selected callee’s normal return. At a boundary member result, the producing
/// invocation identifies the requirement whose provider must establish the
/// returned backing. Its lifetime-related input does not establish containment,
/// and a caller-chosen result has no such input.
/// A named local borrow leads through its defining BorrowPlace to its backing.
/// A direct borrow from an owner is identified by its argument position, access
/// and projected place on the linked call. The retained owner row of its root
/// records custody and moved paths. A root moved out whole has no row; its
/// move names the destination. The checker compares these with the loan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedLoan {
    pub descriptor: crate::PlaceId,
    pub ty: TypeId,
    pub access: crate::ReferenceAccess,
    pub needed_by_callee: bool,
    pub needed_after_completion: bool,
}

/// Custody while the invocation is incomplete. Normal completion runs the
/// caller's cleanup. A moved argument belongs to the named callee formal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedCustody {
    Caller,
    CalleeArgument(usize),
}

/// The descriptor at this path arrived with the structural formal. An empty
/// path selects the formal itself. Each subsequent shared reborrow names the
/// operation that formed it, ordered from that incoming loan to the stored one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddedLoanOrigin {
    pub formal: crate::PlaceId,
    pub path: Vec<crate::PlaceProjection>,
    pub reborrows: Vec<crate::OperationId>,
}

/// One live reference or view descriptor stored inside a retained owner.
/// It is evidence about the owner's contents, not a separate loan row: the
/// owner remains the only custody claim for its storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddedLoan {
    /// Ordered field projections from the owner's root to the descriptor.
    pub path: Vec<crate::PlaceProjection>,
    /// The descriptor's own type, a reference or view shape.
    pub ty: TypeId,
    pub access: crate::ReferenceAccess,
    pub origin: EmbeddedLoanOrigin,
}

/// The owned root with exactly these subtrees removed. The checker composes
/// the carry demands of the live contents and the root's own provenance.
/// `embedded_loans` lists every descriptor in the live contents, in
/// declaration pre-order, and none below a moved subtree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedOwner {
    pub place: crate::PlaceId,
    pub ty: TypeId,
    pub moved: Vec<Vec<crate::PlaceProjection>>,
    pub custody: OwnedCustody,
    pub embedded_loans: Vec<EmbeddedLoan>,
}

/// Caller storage with no ownership claim and no cleanup that stays in the
/// parked activation because the callee or the continuation needs it:
/// directly, as a borrowed call operand, or as the backing of a retained loan
/// whose defining `BorrowPlace` starts at this home. It is either a primitive
/// local's `EstablishPrimitiveLocal` home or copyable structural storage (a
/// `[copy]` record or a fixed array whose leaves are all primitive scalars),
/// which a loan may select below its root. Neither has an owner row. `carry`
/// composes the authored restrictions of the stored schemas. A mutable local's
/// primitive home is also in `MachineBody::mutable_homes`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedHome {
    pub place: crate::PlaceId,
    pub ty: TypeId,
    pub carry: crate::Carry,
    pub needed_by_callee: bool,
    pub needed_after_completion: bool,
}

/// A restoration window that is still open on every arrival at the parked call
/// (`wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration`). The
/// `BorrowedMove` at `opened` vacated `place`, which is captured through its
/// exclusive lender with any named reborrows expanded. Its result is the
/// displaced value, which keeps its own owner row while it stays live. The debt
/// stays on `place` until a store re-establishes it. Resuming neither repeats
/// the move nor publishes a complete owner. A window closed before the call has
/// no row. The checker judges the window; this row only names it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenWindow {
    pub opened: OperationId,
    pub place: crate::ProjectedPlace,
}

/// The result belongs to normal completion. The parked activation contains only
/// incoming arguments and previously established values needed upon completion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuspensionSite {
    pub call: CallSite,
    pub callee: CallTarget,
    pub continuation: CompletionContinuation,
    pub activation: Vec<ActivationValue>,
    pub retained_loans: Vec<RetainedLoan>,
    pub retained_owners: Vec<RetainedOwner>,
    pub retained_homes: Vec<RetainedHome>,
    /// Open windows in the order of their opening moves.
    pub open_windows: Vec<OpenWindow>,
}

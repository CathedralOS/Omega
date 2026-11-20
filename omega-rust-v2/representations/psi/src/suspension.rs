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
/// selected callee’s normal return and the corresponding actual operand.
/// A named local borrow leads through its defining BorrowPlace to its backing.
/// A direct borrow from an owner is identified by its argument position, access
/// and projected place on the linked call. The retained owner keeps that storage live.
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
}

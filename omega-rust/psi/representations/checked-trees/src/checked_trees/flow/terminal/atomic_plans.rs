//! Checked atomic access plans: one normalized atomic event on an exact
//! primitive leaf, planned from the source carrier.
//!
//! The source carrier spells a fetch, swap, or compare-exchange as two
//! statements: the result local (`let prior: T = 0;`, a placeholder) and the
//! carrier assignment (`place = Atomic { model }`). The plan joins them into
//! one event: the result binding is the placeholder local's dense scalar
//! binding, and the operands are the carrier's authored operand expressions,
//! each retained as its own scalar row (`AtomicOperand`) at the carrier
//! statement. A load is one statement, `let v: T = place.load(o);`, and a store
//! is one carrier assignment with no result.
//!
//! The plan names the operation the carrier denotes, never the arithmetic
//! model the checked interpreter replays; lowering emits one Terminal
//! `AtomicAccess` and must not rebuild the model as a separate read, arithmetic
//! and store.

use crate::checked_trees::flow::terminal::{
    CheckedCallScalarArgument, CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralPathSegment,
};
use language_core::atomic::MemoryOrdering;
use typed_trees::types::PrimitiveType;

/// The fetch-family arithmetic a read-modify-write applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckedAtomicReadModifyWrite {
    FetchAdd,
    FetchSub,
    FetchAnd,
    FetchOr,
    FetchXor,
}

/// One normalized atomic event with its orderings and checked operands. Each
/// operand is the scalar row retained at the carrier statement under
/// `CheckedScalarExpressionRole::AtomicOperand` with its event-order ordinal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedAtomicEvent {
    Load {
        ordering: MemoryOrdering,
    },
    Store {
        ordering: MemoryOrdering,
        value: CheckedCallScalarArgument,
    },
    ReadModifyWrite {
        operation: CheckedAtomicReadModifyWrite,
        ordering: MemoryOrdering,
        operand: CheckedCallScalarArgument,
    },
    Swap {
        ordering: MemoryOrdering,
        value: CheckedCallScalarArgument,
    },
    CompareExchange {
        success: MemoryOrdering,
        failure: MemoryOrdering,
        expected: CheckedCallScalarArgument,
        replacement: CheckedCallScalarArgument,
    },
}

impl CheckedAtomicEvent {
    /// Every event but a store observes the resident and binds its prior.
    pub const fn observes_resident(&self) -> bool {
        !matches!(self, Self::Store { .. })
    }

    /// Every event but a load may replace the resident.
    pub const fn modifies_resident(&self) -> bool {
        !matches!(self, Self::Load { .. })
    }

    /// The operands in event order, matching their `AtomicOperand` ordinals.
    pub fn operands(&self) -> Vec<&CheckedCallScalarArgument> {
        match self {
            Self::Load { .. } => Vec::new(),
            Self::Store { value, .. } | Self::Swap { value, .. } => vec![value],
            Self::ReadModifyWrite { operand, .. } => vec![operand],
            Self::CompareExchange {
                expected,
                replacement,
                ..
            } => vec![expected, replacement],
        }
    }
}

/// One atomic event on the scalar field `field_identity` of the record
/// `carrier_path` selects beneath a structural parameter. Atomic cells are
/// record fields, so the location is the same (root, carrier path, field)
/// triple a scalar field store names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedAtomicAccessPlan {
    /// The carrier statement: the assignment for a writing event, the local
    /// for a load. Operand rows are keyed here.
    pub statement_index: u32,
    /// Dense index into the body's structural parameter plans. Planning
    /// admits a modifying event only through exclusive authority; Terminal
    /// verification rechecks it.
    pub parameter_index: u32,
    /// Static fields and at most one literal element from the root to the
    /// record holding the atomic field.
    pub carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    /// The atomic field's Terminal identity within that record.
    pub field_identity: String,
    /// The field's carrier, which every operand and the result share.
    pub primitive_type: PrimitiveType,
    pub event: CheckedAtomicEvent,
    /// The immutable local binding the observed prior, present exactly when
    /// the event observes. Its statement is the placeholder local for a
    /// writing event and the carrier itself for a load.
    pub result: Option<CheckedUnitScalarResultBindingPlan>,
}

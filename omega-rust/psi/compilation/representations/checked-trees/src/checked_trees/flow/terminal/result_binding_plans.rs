//! Result binding plans for scalar and structural results and the
//! byte-sequence field stores that receive them.

use crate::CheckedScalarExpression;
use crate::checked_trees::flow::terminal::CheckedUnitStructuralPathSegment;
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

/// Exact state-local coordinate receiving one primitive boundary result in a
/// Unit-effect body. The local has no structural place or cleanup action; its
/// dense binding ordinal is the scalar value namespace used by later checked
/// call arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedUnitScalarResultBindingPlan {
    pub statement_index: u32,
    pub binding_ordinal: u32,
    pub primitive_type: PrimitiveType,
}

/// Exact whole structural operation result in the shared dense value namespace.
/// Its statement may bind an immutable local or directly supply completion.
/// Claim-free construction carries neither a fabricated claim nor a projected path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralResultBindingPlan {
    pub statement_index: u32,
    pub binding_ordinal: u32,
    pub type_identity: String,
    pub multiplicity: Multiplicity,
}

/// Exact source occurrence of bounded byte-owner replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedByteSequenceStorePlan {
    pub statement_index: u32,
    pub destination_parameter_position: u32,
    pub destination: CheckedByteSequenceStoreDestination,
    pub value: CheckedByteSequenceValuePlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedByteSequenceStoreDestination {
    Whole,
    Field {
        carrier_path: Vec<CheckedUnitStructuralPathSegment>,
        field_identity: String,
    },
}

/// Pure byte values retain authored evaluation order through arena children.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedByteSequenceValuePlan {
    pub nodes: arena::Arena<CheckedByteSequenceValue>,
    pub root: arena::Handle<CheckedByteSequenceValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedByteSequenceValue {
    Literal(Vec<u8>),
    Field {
        parameter_position: u32,
        carrier_path: Vec<CheckedUnitStructuralPathSegment>,
        field_identity: String,
        capacity: u64,
    },
    Concatenation {
        left: arena::Handle<CheckedByteSequenceValue>,
        right: arena::Handle<CheckedByteSequenceValue>,
        capacity: u64,
    },
}

impl Default for CheckedByteSequenceValue {
    fn default() -> Self {
        Self::Literal(Vec::new())
    }
}

/// Exact source operands of a scalar byte replacement within the live prefix.
/// The stored byte is a pure value or the statement's own call result; byte
/// leaves lower no scalar computation, so `Computation` is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralByteSequenceFieldByteStorePlan {
    pub statement_index: u32,
    pub destination_parameter_position: u32,
    pub carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    pub field_identity: String,
    pub index: CheckedScalarExpression,
    pub value: crate::CheckedStoreValue,
}

/// Exact authored operands of a write through a whole mutable byte view. The
/// stored byte is refused as a `Computation` exactly as a byte replacement's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedByteSequenceWritePlan {
    pub statement_index: u32,
    pub destination_parameter_position: u32,
    pub index: CheckedScalarExpression,
    pub value: crate::CheckedStoreValue,
}

/// Exact authored operands of a runtime-indexed element write through a
/// mutable `&mut [T]` view. The destination names the view's root — a
/// borrowed slice parameter or a locally bound `&mut` view — the index its
/// runtime selector, and the value the element-typed scalar stored. The
/// element's primitive type is retained so lowering validates the stored
/// scalar against the view's declared element, never the destination type
/// of another leaf family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedElementViewWritePlan {
    pub statement_index: u32,
    pub destination: crate::CheckedStructuralScalarFieldStoreDestination,
    pub index: CheckedScalarExpression,
    pub element: PrimitiveType,
    pub value: crate::CheckedStoreValue,
}

/// Primitive store custody stays separate from immutable scalar bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedPrimitiveStoreDestination {
    Parameter { parameter_index: u32 },
    Local { symbol: SymbolHandle },
}

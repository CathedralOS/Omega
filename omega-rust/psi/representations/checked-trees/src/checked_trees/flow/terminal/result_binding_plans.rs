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

/// Exact source occurrence of a whole bounded byte-field replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralByteSequenceFieldStorePlan {
    pub statement_index: u32,
    pub destination_parameter_position: u32,
    pub carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    pub field_identity: String,
    /// The first admitted source is a literal; Terminal consumes a shared view.
    pub bytes: Vec<u8>,
}

/// Exact source operands of a scalar byte replacement within the live prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralByteSequenceFieldByteStorePlan {
    pub statement_index: u32,
    pub destination_parameter_position: u32,
    pub carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    pub field_identity: String,
    pub index: CheckedScalarExpression,
    pub value: CheckedScalarExpression,
}

/// Exact authored operands of a write through a whole mutable byte view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedByteSequenceWritePlan {
    pub statement_index: u32,
    pub destination_parameter_position: u32,
    pub index: CheckedScalarExpression,
    pub value: CheckedScalarExpression,
}

/// Primitive store custody stays separate from immutable scalar bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedPrimitiveStoreDestination {
    Parameter { parameter_index: u32 },
    Local { symbol: SymbolHandle },
}

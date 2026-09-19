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

/// The scalar source a byte-sequence store replays. Every byte-store
/// destination lane resolves this once and retains the same distinction the
/// scalar field store carries: a bound pure authored expression, or the SSA
/// result of the scalar call the same statement performs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedByteSequenceStoreValue {
    Pure(CheckedScalarExpression),
    /// The already-defined SSA result of the scalar call this same statement
    /// performs, named by its dense position in the consuming plan's scalar
    /// namespace -- the namespace `CheckedScalarExpression::Local` indexes,
    /// scalar parameters first and then each established scalar result in
    /// order. The authored call binds no local, so no `AssignmentValue`
    /// scalar-expression row exists for it; the receiving lowerer reconstructs
    /// the value from the call operation it already emitted for this
    /// statement instead of from an authored expression.
    ScalarResult {
        position: u32,
    },
}

impl CheckedByteSequenceStoreValue {
    pub fn as_pure(&self) -> Option<&CheckedScalarExpression> {
        match self {
            Self::Pure(expression) => Some(expression),
            Self::ScalarResult { .. } => None,
        }
    }
}

/// Exact source operands of a scalar byte replacement within the live prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralByteSequenceFieldByteStorePlan {
    pub statement_index: u32,
    pub destination_parameter_position: u32,
    pub carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    pub field_identity: String,
    pub index: CheckedScalarExpression,
    pub value: CheckedByteSequenceStoreValue,
}

/// Exact authored operands of a write through a whole mutable byte view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedByteSequenceWritePlan {
    pub statement_index: u32,
    pub destination_parameter_position: u32,
    pub index: CheckedScalarExpression,
    pub value: CheckedByteSequenceStoreValue,
}

/// Primitive store custody stays separate from immutable scalar bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedPrimitiveStoreDestination {
    Parameter { parameter_index: u32 },
    Local { symbol: SymbolHandle },
}

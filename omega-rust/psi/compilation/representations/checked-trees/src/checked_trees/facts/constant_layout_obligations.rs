//! Exact qualified geometry consumed by private proof closure. Source
//! coordinates retain their owners until ordinary Terminal identity lowering.
use symbols::SymbolHandle;
use typed_trees::{expression::ExpressionHandle, name::Identifier};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedLayoutQueryCarrier {
    Unit,
    Scalar(semantic_vocabulary::ScalarType),
    Structural(typed_trees::type_identity::TypeIdentityId),
}

/// A checked query's complete subject, including observations used only by
/// erased contracts. Later closure reads this record, never type source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedLayoutQuery {
    pub expression: ExpressionHandle,
    pub owner: SymbolHandle,
    pub kind: language_core::data_layout_queries::DataLayoutQueryKind,
    pub carrier: CheckedLayoutQueryCarrier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedLayoutOwner {
    Machine(SymbolHandle),
    /// A canonical declaration value. Formation consumes the assignment even
    /// when the value has no executable use; runtime uses retain their own
    /// machine or storage assignments before this checked-only owner erases.
    Constant(SymbolHandle),
    Carrier(SymbolHandle),
    /// A materialized structural carrier, including a recipe-sized array.
    Structural {
        machine: SymbolHandle,
        identity: typed_trees::type_identity::TypeIdentityId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedLayoutRequirement {
    /// The source judgment whose complete observation assignment is retained.
    pub proof_owner: SymbolHandle,
    /// The executable, declaration value or carrier consuming that judgment.
    pub owner: CheckedLayoutOwner,
    pub alternatives: Vec<CheckedLayoutAlternative>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedLayoutAlternative {
    pub target: Identifier,
    pub observations: Vec<CheckedLayoutObservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedLayoutObservation {
    pub expression: ExpressionHandle,
    pub owner: SymbolHandle,
    pub value: u64,
}

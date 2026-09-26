use semantic_vocabulary::{ObligationId, StructuralFieldId, ValueId};

/// One selected case field initialized from an already evaluated scalar.
/// Fields follow declaration order; operand evaluation remains in preceding
/// operations in authored order. A bounded integer field requires its exact
/// declaration-derived range obligation; an unrestricted scalar has none.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarCaseField {
    pub field: StructuralFieldId,
    pub value: ValueId,
    pub range_obligation: Option<ObligationId>,
}

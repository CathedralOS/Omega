use semantic_vocabulary::{ObligationId, StructuralFieldId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarCaseField {
    pub field: StructuralFieldId,
    pub value: ValueId,
    pub range_obligation: Option<ObligationId>,
}

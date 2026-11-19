use semantic_vocabulary::{ObligationId, StructuralFieldId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFieldInitializer {
    pub field: StructuralFieldId,
    pub value: RecordFieldValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordFieldValue {
    Scalar {
        value: ValueId,
        range_obligation: Option<ObligationId>,
    },
    Structural(crate::StructuralArgument),
}

impl RecordFieldValue {
    pub fn as_structural(&self) -> Option<&crate::StructuralArgument> {
        let RecordFieldValue::Structural(structural) = self else {
            return None;
        };
        Some(structural)
    }
}

use semantic_vocabulary::{ObligationId, StructuralFieldId, ValueId};

/// One declaration-ordered field of a complete record. Operand-producing
/// operations retain authored evaluation order before atomic establishment.
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
    /// An exact completed owned child; construction does not borrow it or
    /// rediscover its contents from a same-shaped declaration.
    Structural(crate::StructuralArgument),
}

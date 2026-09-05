use optimization_unit::TotalScalarIdentityKind;
use semantic_vocabulary::{IntegerType, IntegerValue, OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TotalScalarIdentityShape {
    pub source_operation: OperationId,
    pub result: ValueId,
    pub replacement: ValueId,
    pub law_operand: ValueId,
    pub scalar_type: IntegerType,
    pub law_operand_type: IntegerType,
    pub identity: TotalScalarIdentityKind,
    pub expected_law_value: IntegerValue,
}

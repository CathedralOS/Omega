//! Condition custody returned by one exact producer leaf.

use super::*;

pub(in crate::legalization) struct DerivedCondition<'a> {
    pub source: ValueId,
    pub legalized: LegalizedCondition,
    pub shape: ScalarConditionShape,
    pub result_type: IntegerType,
    pub when_true: &'a TargetConditionalIntegerArm,
    pub when_false: &'a TargetConditionalIntegerArm,
    pub conditional_node_index: usize,
    pub provenance_operations: Vec<OperationId>,
}

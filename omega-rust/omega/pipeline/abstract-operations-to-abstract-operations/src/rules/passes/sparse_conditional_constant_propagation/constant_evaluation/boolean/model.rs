//! Closed operation-kind vocabulary shared by Boolean-result constant folds.

use optimization_unit::IntegerEvaluationWitness;
use semantic_vocabulary::{OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BooleanEvaluationKind {
    Not,
    Equal,
    IntegerEqual,
    IntegerLessThan,
    IntegerLessOrEqual,
}

pub(super) struct BooleanEvaluation {
    pub source_operation: OperationId,
    pub result: ValueId,
    pub constant: bool,
    pub witness: IntegerEvaluationWitness,
}

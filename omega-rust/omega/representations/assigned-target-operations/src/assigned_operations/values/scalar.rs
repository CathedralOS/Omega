//! values scalar in the assigned operations program.

use crate::AssignedBooleanExpression;
use crate::AssignedIntegerExpression;
use semantic_vocabulary::IntegerType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedScalarExpression {
    Boolean(AssignedBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        expression: AssignedIntegerExpression,
    },
}

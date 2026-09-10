use arena::HandleSpan;
use typed_trees::expression::ExpressionHandle;

use super::FlowConstraintRef;

/// Facts at completion of one authored operator operand. Scalar copies keep
/// this value's facts even if a later operand changes its former source storage;
/// other carriers additionally need the same facts live at invocation until
/// exact payload and referent custody supports more precise transport.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowOperatorOperandFact {
    pub expression: ExpressionHandle,
    pub constraints: HandleSpan<FlowConstraintRef>,
}

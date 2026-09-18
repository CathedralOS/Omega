use arena::HandleSpan;
use typed_trees::expression::ExpressionHandle;

use super::FlowConstraintRef;

/// Facts at completion of one authored operator operand. Scalar copies and
/// by-value carriers with stable observable contents keep this value's facts
/// even if a later operand changes its former source storage — the bound
/// operand is a detached copy. View carriers such as references and slices
/// additionally need the same facts live at invocation until exact referent
/// custody supports more precise transport.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowOperatorOperandFact {
    pub expression: ExpressionHandle,
    pub constraints: HandleSpan<FlowConstraintRef>,
}

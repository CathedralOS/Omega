//! calls in the current typed trees program.

pub mod boundary;
pub mod dynamic_traits;
pub mod service;
pub mod signature;

impl super::TypedTrees {
    /// A data-qualified call names its owner without evaluating a receiver.
    /// A value path, borrow, or other expression is still a runtime receiver.
    pub fn call_has_no_runtime_receiver(
        &self,
        call: &crate::expression::TableCallExpression,
        owner: &crate::machine::Machine,
        target: &crate::state::State,
    ) -> bool {
        if call.target_symbol != target.symbol
            || self
                .state_parameters(target)
                .iter()
                .any(|parameter| parameter.is_self)
        {
            return false;
        }
        if !call.receiver.is_valid() {
            return true;
        }
        if !owner.attached_data_symbol.is_valid()
            || !self.expression_table.expression_is_valid(call.receiver)
        {
            return false;
        }
        let crate::expression::ExpressionNode::Name(path) =
            self.expression_table.expression(call.receiver)
        else {
            return false;
        };
        path.symbol == owner.attached_data_symbol
    }
}

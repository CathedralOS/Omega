//! calls in the current typed trees program.

pub mod boundary;
pub mod dynamic_traits;
pub mod finite_family;
pub mod service;
pub mod signature;

impl super::TypedTrees {
    /// Rejoin the source qualifier to its exact selected callable declaration.
    /// Bodyless named operators and boundary requirements have no machine state;
    /// limiting this query to machine bodies invents a read of their namespace
    /// during nested argument evaluation. Keep unknown or instance qualifiers
    /// observable, and leave actual operands on the ordinary ordered read path.
    pub fn call_has_no_runtime_receiver_for_target(
        &self,
        call: &crate::expression::TableCallExpression,
    ) -> bool {
        if let Some(owner) = self.machine_holding_state(call.target_symbol)
            && let Some(target) = self
                .machine_states(owner)
                .iter()
                .find(|target| target.symbol == call.target_symbol)
        {
            return self.call_has_no_runtime_receiver(call, owner, target);
        }
        if crate::operator::selected_named_expression_call_has_no_runtime_receiver(self, call) {
            return true;
        }
        let mut requirements = self
            .traits()
            .iter()
            .filter(|owner| owner.is_boundary)
            .flat_map(|owner| {
                self.trait_machine_signatures(owner)
                    .iter()
                    .filter(move |signature| signature.symbol == call.target_symbol)
                    .map(move |signature| (owner, signature))
            });
        let Some((owner, signature)) = requirements.next() else {
            return false;
        };
        let parameters = self.state_signature_parameters(signature);
        let arguments = self.expression_table.expression_handles(call.arguments);
        if requirements.next().is_some()
            || !call.target_symbol.is_valid()
            || arguments.len() != call.arguments.count() as usize
            || arguments.len() != parameters.len()
            || parameters.iter().any(|parameter| parameter.is_self)
            || call.target.as_str() != signature.name.as_str()
        {
            return false;
        }
        if !call.receiver.is_valid() {
            return true;
        }
        if !self.expression_table.expression_is_valid(call.receiver) {
            return false;
        }
        matches!(self.expression_table.expression(call.receiver),
            crate::expression::ExpressionNode::Name(path) if path.symbol == owner.symbol)
    }

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

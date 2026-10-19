//! calls in the current typed trees program.

pub mod boundary;
pub mod dynamic_traits;
pub mod finite_family;
pub mod service;
pub mod signature;
pub mod static_applications;

impl super::TypedTrees {
    /// Rejoin the source qualifier to its exact selected callable declaration.
    /// Bodyless named operators and trait requirements have no machine state;
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
        let slot = self.dynamic_slot_by_symbol(call.target_symbol);
        let requirement = slot.map_or(call.target_symbol, |slot| slot.requirement);
        let mut requirements = self
            .traits()
            .iter()
            .filter(|owner| slot.is_none_or(|slot| slot.declaring_trait == owner.symbol))
            .filter_map(|owner| {
                // Authored and closed applied requirements have the same
                // declaration namespace. The owner query retains that exact
                // correspondence; scanning authored rows alone invented a
                // runtime read of a generic requirement's qualifier.
                self.trait_machine_signature_by_symbol(owner, requirement)
                    .map(|signature| (owner, signature))
            });
        let Some((owner, signature)) = requirements.next() else {
            return false;
        };
        let parameters = self.state_signature_parameters(signature);
        let operand_count = match slot {
            Some(slot) if slot.data_parameters().len() == parameters.len() => slot.parameters.len(),
            Some(_) => return false,
            None => parameters.len(),
        };
        let arguments = self.expression_table.expression_handles(call.arguments);
        if requirements.next().is_some()
            || !call.target_symbol.is_valid()
            || arguments.len() != call.arguments.count() as usize
            || arguments.len() != operand_count
            || (slot.is_none() && call.target.as_str() != signature.name.as_str())
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

    /// A data- or implementation-qualified call names its owner without
    /// evaluating a receiver. An implementation name (a conformance or an
    /// evidence binder) identifies a map of machines, never a value that can
    /// be borrowed as state, so qualifying a row call by it supplies no
    /// argument. A value path, borrow, or other expression is still a runtime
    /// receiver.
    pub fn call_has_no_runtime_receiver(
        &self,
        call: &crate::expression::TableCallExpression,
        owner: &crate::machine::Machine,
        target: &crate::state::State,
    ) -> bool {
        if call.target_symbol != target.symbol {
            return false;
        }
        if !call.receiver.is_valid() {
            return true;
        }
        if !self.expression_table.expression_is_valid(call.receiver) {
            return false;
        }
        let crate::expression::ExpressionNode::Name(path) =
            self.expression_table.expression(call.receiver)
        else {
            return false;
        };
        if path.symbol.is_valid()
            && matches!(
                self.symbols.get(path.symbol).kind,
                symbols::SymbolKind::Conformance | symbols::SymbolKind::ConformanceParameter
            )
        {
            return true;
        }
        owner.attached_data_symbol.is_valid() && path.symbol == owner.attached_data_symbol
    }
}

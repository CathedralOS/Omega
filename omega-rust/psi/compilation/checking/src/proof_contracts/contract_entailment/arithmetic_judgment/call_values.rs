//! Call denotations retain their supplying declaration and complete operands.
//! Reserving a value publishes no precondition, result bound or guarantee;
//! application formation owns those decisions after its children have formed.

use super::{Engine, OpaqueTerm, Polynomial};
use crate::frozen_program::expression_table;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

impl Engine<'_> {
    pub(super) fn selected_call_value(
        &mut self,
        owner: SymbolHandle,
        expression: ExpressionHandle,
        target: SymbolHandle,
        arguments: Vec<Polynomial>,
    ) -> Option<Polynomial> {
        if !owner.is_valid() || !target.is_valid() {
            return None;
        }
        let call = expression_table(self.program).call_node(expression)?;
        let selected =
            super::super::specification_calls::selected_call_contract(self.program, owner, call)?;
        if super::super::specification_calls::owner_symbol(&selected) != target
            || expression_table(self.program)
                .expression_handles(call.arguments)
                .len()
                != arguments.len()
        {
            return None;
        }
        let atom = format!("\0applied-call:{owner:?}:{expression:?}:{target:?}:{arguments:?}");
        self.register_opaque_term(
            atom.clone(),
            OpaqueTerm::Call {
                owner,
                expression,
                target,
                arguments,
            },
        );
        Some(Polynomial::atom(atom))
    }
}

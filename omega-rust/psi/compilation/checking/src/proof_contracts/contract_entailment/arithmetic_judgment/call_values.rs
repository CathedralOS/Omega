//! Call denotations retain their supplying declaration and complete operands.
//! Reserving a value publishes no precondition, result bound or guarantee;
//! application formation owns those decisions after its children have formed.

use super::{BinaryOperator, Engine, OpaqueTerm, Polynomial};
use crate::frozen_program::expression_table;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

impl Engine<'_> {
    /// Newly established equalities can change an applied operand's normal
    /// form. Preserve congruence with the previously reserved call value;
    /// neither representation supplies an additional result guarantee.
    pub(super) fn refresh_call_congruences(
        &mut self,
        comparisons: &mut Vec<(BinaryOperator, Polynomial, Polynomial)>,
    ) -> bool {
        for position in 0..self.opaque_terms.len() {
            let (atom, operation) = self.opaque_terms[position].clone();
            let OpaqueTerm::Call {
                owner,
                expression,
                target,
                arguments,
            } = operation
            else {
                continue;
            };
            let normalized = arguments
                .iter()
                .map(|value| self.substituted(value))
                .collect::<Vec<_>>();
            if normalized == arguments {
                continue;
            }
            let Some(value) = self.selected_call_value(owner, expression, target, normalized)
            else {
                return false;
            };
            let previous = self.substituted(&Polynomial::atom(atom.clone()));
            let canonical = self.substituted(&value);
            if previous == canonical {
                continue;
            }
            // Keep residual equations even when no acyclic substitution is
            // possible. Congruence can expose contradictory public facts or
            // a constraint such as c == 2*c; a cyclic alias would lose that
            // obligation and repeatedly expand during normalization.
            self.harvest_substitution(&previous, &canonical);
            comparisons.push((BinaryOperator::Equal, previous, canonical));
        }
        true
    }

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

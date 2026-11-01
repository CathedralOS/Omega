//! Call denotations retain their supplying declaration and complete operands.
//! Reserving a value publishes no precondition, result bound or guarantee;
//! application formation owns those decisions after its children have formed.

use super::{BinaryOperator, Engine, IntegerApplicationInput, OpaqueTerm, Polynomial};
use crate::frozen_program::expression_table;
use std::sync::atomic::Ordering;
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
            let (_, operation) = &self.opaque_terms[position];
            let value = match operation {
                OpaqueTerm::Call {
                    owner,
                    expression,
                    target,
                    arguments,
                } => {
                    let normalized = arguments
                        .iter()
                        .map(|input| input.mapped(|value| Some(self.substituted(value))))
                        .collect::<Option<Vec<_>>>();
                    let Some(normalized) = normalized else {
                        return false;
                    };
                    if normalized == *arguments {
                        continue;
                    }
                    let (owner, expression, target) = (*owner, *expression, *target);
                    self.selected_call_value(owner, expression, target, normalized)
                }
                OpaqueTerm::DeclarationApplication { measure, input } => {
                    let Some(normalized) = input.mapped(|value| Some(self.substituted(value)))
                    else {
                        return false;
                    };
                    if normalized == *input {
                        continue;
                    }
                    self.declared_application_value(*measure, normalized)
                }
                _ => continue,
            };
            let atom = self.opaque_terms[position].0.clone();
            let Some(value) = value else {
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

    /// Reserve the exact supplying declaration and completed input. This
    /// publishes no result bound, branch fact or formation decision.
    pub(in crate::proof_contracts::contract_entailment) fn declared_application_value(
        &mut self,
        measure: SymbolHandle,
        input: impl Into<IntegerApplicationInput>,
    ) -> Option<Polynomial> {
        self.program
            .measures()
            .iter()
            .find(|definition| definition.symbol == measure)?;
        let input = input.into().mapped(|value| Some(self.substituted(value)))?;
        if let Some((atom, _)) = self.opaque_terms.iter().find(|(_, operation)| {
            matches!(operation, OpaqueTerm::DeclarationApplication { measure: existing, input: value }
                if *existing == measure && *value == input)
        }) { return Some(Polynomial::atom(atom.clone())); }
        let identity = self
            .denotation_identities
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .ok()?;
        let atom = format!("\0declared-application:{identity}");
        self.register_opaque_term(
            atom.clone(),
            OpaqueTerm::DeclarationApplication { measure, input },
        );
        Some(Polynomial::atom(atom))
    }

    pub(super) fn selected_call_value(
        &mut self,
        owner: SymbolHandle,
        expression: ExpressionHandle,
        target: SymbolHandle,
        arguments: Vec<IntegerApplicationInput>,
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
        if let Some((atom, _)) = self.opaque_terms.iter().find(|(_, operation)| {
            matches!(operation, OpaqueTerm::Call { owner: existing_owner, expression: existing_expression,
                target: existing_target, arguments: existing_arguments }
                if *existing_owner == owner && *existing_expression == expression
                && *existing_target == target && *existing_arguments == arguments)
        }) { return Some(Polynomial::atom(atom.clone())); }
        let identity = self
            .denotation_identities
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .ok()?;
        let atom = format!("\0applied-call:{identity}");
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

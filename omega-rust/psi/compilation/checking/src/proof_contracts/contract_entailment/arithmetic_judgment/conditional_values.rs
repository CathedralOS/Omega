//! Exact conditional values retain selected-path predicates beside their values.
//! These are denotations, not formation certificates or ambient premises. A
//! lower-bound query splits only conditional dependencies of its actual goal,
//! and must prove every feasible arm under that arm's captured predicates.

use super::{BigInt, BinaryOperator, Engine, OpaqueTerm, Polynomial};
use std::collections::BTreeSet;
use std::sync::atomic::Ordering;

pub(super) type Comparison = (BinaryOperator, Polynomial, Polynomial);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::proof_contracts::contract_entailment) struct ConditionalValueArm {
    pub(super) conditions: Vec<Comparison>,
    pub(super) value: Polynomial,
}

impl ConditionalValueArm {
    pub(super) fn visit_operands(&self, mut visit: impl FnMut(&Polynomial)) {
        for (_, left, right) in &self.conditions {
            visit(left);
            visit(right);
        }
        visit(&self.value);
    }

    pub(super) fn mapped(
        &self,
        mut map: impl FnMut(&Polynomial) -> Option<Polynomial>,
    ) -> Option<Self> {
        Some(Self {
            conditions: self
                .conditions
                .iter()
                .map(|(operator, left, right)| Some((*operator, map(left)?, map(right)?)))
                .collect::<Option<Vec<_>>>()?,
            value: map(&self.value)?,
        })
    }
}

impl Engine<'_> {
    /// The formation owner establishes coverage and independent arm formation.
    /// Re-minting a captured definition imports neither old bounds nor proofs.
    pub(super) fn conditional_value(
        &mut self,
        arms: Vec<ConditionalValueArm>,
    ) -> Option<Polynomial> {
        if arms.is_empty() {
            return None;
        }
        if arms.iter().all(|arm| arm.value == arms[0].value) {
            return Some(arms[0].value.clone());
        }
        if let Some((atom, _)) = self.opaque_terms.iter().find(|(_, operation)| {
            matches!(operation, OpaqueTerm::Conditional { arms: existing } if existing == &arms)
        }) {
            return Some(Polynomial::atom(atom.clone()));
        }
        let identity = self
            .denotation_identities
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .ok()?;
        let atom = format!("\0conditional:{identity}");
        self.register_opaque_term(atom.clone(), OpaqueTerm::Conditional { arms });
        Some(Polynomial::atom(atom))
    }

    /// Import only operation definitions from a selected branch. Its bounds,
    /// substitutions and result guarantees stay local; normalized operands carry
    /// their exact mathematical consequences without granting global authority.
    pub(super) fn retain_operation_definitions(&mut self, branch: &Self) -> Option<()> {
        if !std::sync::Arc::ptr_eq(&self.denotation_identities, &branch.denotation_identities) {
            return None;
        }
        for (atom, operation) in &branch.opaque_terms {
            self.register_opaque_term(atom.clone(), operation.clone());
        }
        Some(())
    }

    /// A compiler-query allowance, never executable fuel or a language bound.
    /// Clones share the invocation allowance so shallow Boolean width cannot
    /// allocate an unbounded path tree. Exhaustion proves nothing. Charging
    /// captured facts/definitions also bounds the state copied at each fork.
    pub(super) fn charge_conditional_work(&self) -> Option<()> {
        let cost = 1usize
            .checked_add(self.bounds.len())?
            .checked_add(self.opaque_terms.len())?;
        self.conditional_work_remaining
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                remaining.checked_sub(cost)
            })
            .ok()
            .map(|_| ())
    }

    pub(super) fn conditional_lower_bound(
        &self,
        polynomial: &Polynomial,
        bound: &BigInt,
    ) -> Option<bool> {
        if self.conditional_terms.is_empty() {
            return None;
        }
        let polynomial = self.substituted(polynomial);
        let mut dependencies: BTreeSet<String> = polynomial
            .terms
            .keys()
            .flat_map(|monomial| monomial.keys().cloned())
            .collect();
        for (atom, operation) in self.opaque_terms.iter().rev() {
            if dependencies.contains(atom) {
                operation.visit_operands(|operand| {
                    dependencies.extend(
                        operand
                            .terms
                            .keys()
                            .flat_map(|monomial| monomial.keys().cloned()),
                    );
                });
            }
        }
        let (atom, arms) = self.opaque_terms.iter().find_map(|(atom, operation)| {
            if !dependencies.contains(atom)
                || self.substituted(&Polynomial::atom(atom.clone()))
                    != Polynomial::atom(atom.clone())
            {
                return None;
            }
            let OpaqueTerm::Conditional { arms } = operation else {
                return None;
            };
            Some((atom, arms))
        })?;
        Some(arms.iter().all(|arm| {
            if self.charge_conditional_work().is_none() {
                return false;
            }
            let mut selected = self.clone();
            // Definition dependencies precede their result in mint order; this
            // equation cannot recursively mention the conditional being selected.
            selected.harvest_substitution(&Polynomial::atom(atom.clone()), &arm.value);
            if selected.integer_comparisons_unsatisfiable(arm.conditions.clone()) {
                return true;
            }
            selected.prove_at_least(&selected.substituted(&polynomial), bound)
        }))
    }
}

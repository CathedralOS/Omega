//! Equality transport at one program point, shared by proof search and
//! private-input description. A taken Boolean comparison can expose the value
//! of another Boolean, including a join parameter: `(b == true) == true`
//! establishes `b == true` before a predicate about `b` crosses a call.
//!
//! Every derived form retains its ordinary transport evidence and its original
//! premises. Definitions precede branch literals; a newly derived literal only
//! fills an undefined value. Dense value users revisit affected predicates,
//! and each value gains at most one definition. This computes no new domain,
//! authority or assumption, and final obligation discharge still checks the
//! evidence with the kernel.

use std::collections::VecDeque;

use proof_admission::{ProofNode, ProofRule, check_value_equality_denotation};
use semantic_vocabulary::{
    IdTable, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};
use terminal_check_tables::values::{ValueOrigin, ValueRow};

/// One hypothesis or one conjunct of it, with the node that yields it.
pub(crate) struct Fact {
    pub(crate) proposition: Proposition,
    pub(crate) proof: ProofNode,
    /// The index of a whole semantic axiom, cited by definition-bound rules.
    pub(crate) axiom: Option<usize>,
}

pub(crate) struct Known {
    pub(crate) facts: Vec<Fact>,
    pub(crate) equations: Vec<(Proposition, ProofNode)>,
}

fn flatten(
    proposition: &Proposition,
    proof: ProofNode,
    axiom: Option<usize>,
    facts: &mut Vec<Fact>,
) {
    if let Proposition::Conjunction(parts) = proposition {
        for (conjunct, part) in parts.iter().enumerate() {
            flatten(
                part,
                ProofNode {
                    conclusion: part.clone(),
                    rule: ProofRule::ConjunctionElimination {
                        conjunction: Box::new(proof.clone()),
                        conjunct,
                    },
                },
                None,
                facts,
            );
        }
    }
    facts.push(Fact {
        proposition: proposition.clone(),
        proof,
        axiom,
    });
}

pub(crate) fn is_branch_fact(proposition: &Proposition) -> bool {
    matches!(
        proposition,
        Proposition::Equal(
            ScalarTerm::Value {
                scalar_type: ScalarType::Boolean,
                ..
            },
            ScalarTerm::Boolean(_),
        )
    )
}

impl Known {
    pub(crate) fn new(
        context: &PropositionContext,
        assumptions: &[Proposition],
        axioms: &[Proposition],
        values: &IdTable<ValueId, ValueRow>,
    ) -> Self {
        let mut facts = Vec::new();
        for (index, assumption) in assumptions.iter().enumerate() {
            flatten(
                assumption,
                ProofNode {
                    conclusion: assumption.clone(),
                    rule: ProofRule::Assumption { index },
                },
                None,
                &mut facts,
            );
        }
        for (index, axiom) in axioms.iter().enumerate() {
            flatten(
                axiom,
                ProofNode {
                    conclusion: axiom.clone(),
                    rule: ProofRule::SemanticAxiom { index },
                },
                Some(index),
                &mut facts,
            );
        }
        let equations: Vec<_> = facts
            .iter()
            .filter(|fact| match &fact.proposition {
                Proposition::Equal(subject @ ScalarTerm::Value { .. }, definition) => {
                    match definition {
                        ScalarTerm::Boolean(_) => fact.axiom.is_some(),
                        ScalarTerm::Integer { .. } => true,
                        // A read with a checked initial-content equation:
                        // the available axiom ties this exact immutable value
                        // to its formal or producing call. Join scope is
                        // checked separately against the owning root.
                        ScalarTerm::Value { id, .. }
                            if fact.axiom.is_some()
                                && matches!(
                                    values.get(*id).map(|row| row.origin),
                                    Some(ValueOrigin::InitialContent { .. })
                                ) =>
                        {
                            true
                        }
                        _ => subject.scalar_type() == ScalarType::Boolean,
                    }
                }
                _ => false,
            })
            .map(|fact| (fact.proposition.clone(), fact.proof.clone()))
            .collect();
        let mut result = Self { facts, equations };
        let mut defined = values.map(|_, _| None);
        for (index, (equation, _)) in result.equations.iter().enumerate() {
            if let Proposition::Equal(ScalarTerm::Value { id, .. }, _) = equation
                && let Some(defined) = defined.get_mut(*id)
            {
                defined.get_or_insert(index);
            }
        }
        for index in 0..result.facts.len() {
            let _ = result.add_literal(index, &mut defined);
        }
        result.normalize(context, &mut defined);
        result
    }

    fn add_literal(
        &mut self,
        index: usize,
        defined: &mut IdTable<ValueId, Option<usize>>,
    ) -> Option<ValueId> {
        let fact = &self.facts[index];
        if !is_branch_fact(&fact.proposition) {
            return None;
        }
        let Proposition::Equal(ScalarTerm::Value { id, .. }, _) = &fact.proposition else {
            return None;
        };
        let definition = defined.get_mut(*id)?;
        if definition.is_some() {
            return None;
        }
        *definition = Some(self.equations.len());
        self.equations
            .push((fact.proposition.clone(), fact.proof.clone()));
        Some(*id)
    }

    /// Cite the first definition of each value the predicate actually uses,
    /// following definitions transitively. Including every earlier derived
    /// equality would duplicate unrelated proof trees at every new step.
    fn transport_inputs(
        &self,
        proposition: &Proposition,
        defined: &IdTable<ValueId, Option<usize>>,
        used: &mut Vec<bool>,
        inputs: &mut Vec<usize>,
        pending: &mut Vec<ValueId>,
    ) {
        // Reset only the preceding traversal's selected definitions. New
        // equations append, so the invocation-owned marks only grow.
        for index in inputs.drain(..) {
            used[index] = false;
        }
        used.resize(self.equations.len(), false);
        proposition.visit_value_ids(|value| pending.push(value));
        while let Some(value) = pending.pop() {
            let Some(Some(index)) = defined.get(value) else {
                continue;
            };
            if std::mem::replace(&mut used[*index], true) {
                continue;
            }
            inputs.push(*index);
            if let Proposition::Equal(_, definition) = &self.equations[*index].0 {
                definition.visit_value_ids(|value| {
                    pending.push(value);
                    true
                });
            }
        }
        inputs.sort_unstable();
        // The transport rule requires an equality even for a closed premise.
        // Its first original equality suffices; it supplies no new fact.
        if inputs.is_empty() && !self.equations.is_empty() {
            used[0] = true;
            inputs.push(0);
        }
    }

    fn normalize(
        &mut self,
        context: &PropositionContext,
        defined: &mut IdTable<ValueId, Option<usize>>,
    ) {
        if self.equations.is_empty() {
            return;
        }
        // Work items remain original premises. Derived facts retain evidence
        // but do not start a second traversal of the same predicate.
        let mut queued: Vec<_> = self.facts.iter().map(|fact| fact.axiom.is_none()).collect();
        let mut pending: VecDeque<_> = queued
            .iter()
            .enumerate()
            .filter_map(|(index, queued)| queued.then_some(index))
            .collect();
        let mut users = defined.map(|_, _| Vec::new());
        let mut used_equations = Vec::new();
        let mut inputs = Vec::new();
        let mut pending_values = Vec::new();
        while let Some(index) = pending.pop_front() {
            queued[index] = false;
            let fact = &self.facts[index];
            self.transport_inputs(
                &fact.proposition,
                defined,
                &mut used_equations,
                &mut inputs,
                &mut pending_values,
            );
            let Ok(transported) = check_value_equality_denotation(
                context,
                &fact.proposition,
                inputs.iter().map(|&index| &self.equations[index].0),
            ) else {
                continue;
            };
            // Unfolding has already exposed the dependencies on undefined
            // values, including those hidden behind operation-result values.
            transported.visit_value_ids(|id| {
                if defined.get(id) == Some(&None)
                    && let Some(users) = users.get_mut(id)
                    && users.last() != Some(&index)
                {
                    users.push(index);
                }
            });
            if transported == fact.proposition {
                continue;
            }
            // Capture only the equations available before this derivation.
            // Adding its consequence below cannot make it its own premise.
            let proof = ProofNode {
                conclusion: transported.clone(),
                rule: ProofRule::ValueEqualityTransport {
                    premise: Box::new(fact.proof.clone()),
                    equalities: inputs
                        .iter()
                        .map(|&index| self.equations[index].1.clone())
                        .collect(),
                },
            };
            let first = self.facts.len();
            flatten(&transported, proof, None, &mut self.facts);
            for derived in first..self.facts.len() {
                if let Some(value) = self.add_literal(derived, defined)
                    && let Some(users) = users.get_mut(value)
                {
                    for user in users.drain(..) {
                        if !std::mem::replace(&mut queued[user], true) {
                            pending.push_back(user);
                        }
                    }
                }
            }
        }
    }
}

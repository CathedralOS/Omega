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
    IdTable, IntegerCarrier, IntegerMathTerm, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};
use terminal_check_tables::values::{ValueOrigin, ValueRow};
use terminal_psi::PrimitiveJudgment;

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
    /// The first checked definition of each value, retained from normalization
    /// so proof transport selects the same reachable equation family.
    pub(crate) definitions: IdTable<ValueId, Option<usize>>,
}

/// One context's reusable equation traversal. Queries clear only the preceding
/// reachable family, rather than allocating and zeroing the full roster again.
pub(crate) struct TransportInputs {
    used: Vec<bool>,
    pub(crate) selected: Vec<usize>,
    pending: Vec<ValueId>,
}

impl TransportInputs {
    pub(crate) fn new(equations: usize) -> Self {
        Self {
            used: vec![false; equations],
            selected: Vec::new(),
            pending: Vec::new(),
        }
    }
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

/// Integer equality and equality of its embeddings have the same meaning.
/// Reuse the kernel's existing equality composition/normalization rule rather
/// than treating a callee's value equality as a new definition or assumption.
fn integer_equality(
    proposition: &Proposition,
    proof: &ProofNode,
) -> Option<(Proposition, ProofNode)> {
    let Proposition::Equal(left, right) = proposition else {
        return None;
    };
    for operand in [left, right] {
        let integer = operand.scalar_type().as_integer()?;
        if integer.carrier() != IntegerCarrier::Fixed || integer.is_address() {
            return None;
        }
    }
    let mut first = IntegerMathTerm::embed(left.clone())?;
    let mut second = IntegerMathTerm::embed(right.clone())?;
    if first > second {
        std::mem::swap(&mut first, &mut second);
    }
    let conclusion = Proposition::IntegerMathEqual(first, second);
    let reflexive = ProofNode {
        conclusion: Proposition::Equal(right.clone(), right.clone()),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let lifted = ProofNode {
        conclusion: conclusion.clone(),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(proof.clone()),
            middle_equals_right: Box::new(reflexive),
        },
    };
    Some((conclusion, lifted))
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
                        ScalarTerm::Value { id, .. } => {
                            subject.scalar_type() == ScalarType::Boolean
                                || (fact.axiom.is_some()
                                    && matches!(
                                        values.get(*id).map(|row| row.origin),
                                        Some(ValueOrigin::InitialContent { .. })
                                    ))
                        }
                        // Actual integer operation equations retain their
                        // selected meaning. The kernel's transport lifts exact
                        // arithmetic and leaves policy arithmetic embedded;
                        // neither becomes an unchecked mathematical rewrite.
                        _ => {
                            subject.scalar_type() == ScalarType::Boolean
                                || (fact.axiom.is_some()
                                    && matches!(subject.scalar_type(), ScalarType::Integer(_)))
                        }
                    }
                }
                _ => false,
            })
            .map(|fact| (fact.proposition.clone(), fact.proof.clone()))
            .collect();
        let mut result = Self {
            facts,
            equations,
            definitions: IdTable::default(),
        };
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
        result.integer_equalities(0, &defined);
        result.normalize(context, &mut defined);
        result.definitions = defined;
        result
    }

    /// Selected definitions already supply their equality through transport.
    /// Only other integer facts need an embedding view, including guarantees
    /// nested inside a conjunction or retained as an axiom. Definition lookup
    /// uses the prepared value index, not a scan or a proof-tree classification.
    fn integer_equalities(&mut self, first: usize, defined: &IdTable<ValueId, Option<usize>>) {
        let end = self.facts.len();
        for index in first..end {
            let fact = &self.facts[index];
            if let Proposition::Equal(ScalarTerm::Value { id, .. }, _) = &fact.proposition
                && let Some(Some(definition)) = defined.get(*id)
                && self.equations[*definition].0 == fact.proposition
            {
                continue;
            }
            if let Some((proposition, proof)) = integer_equality(&fact.proposition, &fact.proof) {
                self.facts.push(Fact {
                    proposition,
                    proof,
                    axiom: None,
                });
            }
        }
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
    pub(crate) fn transport_inputs(
        equations: &[(Proposition, ProofNode)],
        proposition: &Proposition,
        defined: &IdTable<ValueId, Option<usize>>,
        scratch: &mut TransportInputs,
    ) {
        let TransportInputs {
            used,
            selected: inputs,
            pending,
        } = scratch;
        // Reset only the preceding traversal's selected definitions. New
        // equations append, so the invocation-owned marks only grow.
        for index in inputs.drain(..) {
            used[index] = false;
        }
        used.resize(equations.len(), false);
        proposition.visit_value_ids(|value| pending.push(value));
        while let Some(value) = pending.pop() {
            let Some(Some(index)) = defined.get(value) else {
                continue;
            };
            if std::mem::replace(&mut used[*index], true) {
                continue;
            }
            inputs.push(*index);
            if let Proposition::Equal(_, definition) = &equations[*index].0 {
                definition.visit_value_ids(|value| {
                    pending.push(value);
                    true
                });
            }
        }
        inputs.sort_unstable();
        // The transport rule requires an equality even for a closed premise.
        // Its first original equality suffices; it supplies no new fact.
        if inputs.is_empty() && !equations.is_empty() {
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
        let mut transport = TransportInputs::new(self.equations.len());
        while let Some(index) = pending.pop_front() {
            queued[index] = false;
            let fact = &self.facts[index];
            Self::transport_inputs(&self.equations, &fact.proposition, defined, &mut transport);
            let Ok(transported) = check_value_equality_denotation(
                context,
                &fact.proposition,
                transport
                    .selected
                    .iter()
                    .map(|&index| &self.equations[index].0),
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
                    equalities: transport
                        .selected
                        .iter()
                        .map(|&index| self.equations[index].1.clone())
                        .collect(),
                },
            };
            let first = self.facts.len();
            flatten(&transported, proof, None, &mut self.facts);
            self.integer_equalities(first, defined);
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

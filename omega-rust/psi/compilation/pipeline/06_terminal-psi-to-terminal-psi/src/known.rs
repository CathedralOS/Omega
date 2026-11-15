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

/// Declaration terms have logical inputs even when they have no executable
/// producer. The prover borrows their types and observation status from the
/// owner that supplies the inputs.
pub(crate) trait ProofValues {
    fn count(&self) -> usize;
    fn scalar_type(&self, value: ValueId) -> Option<ScalarType>;
    fn is_storage_content(&self, value: ValueId) -> bool;
}

impl ProofValues for IdTable<ValueId, ValueRow> {
    fn count(&self) -> usize {
        self.len()
    }

    fn scalar_type(&self, value: ValueId) -> Option<ScalarType> {
        self.get(value).map(|row| row.scalar_type)
    }

    fn is_storage_content(&self, value: ValueId) -> bool {
        self.get(value)
            .is_some_and(|row| matches!(row.origin, ValueOrigin::StorageContent { .. }))
    }
}

#[cfg(test)]
#[path = "known/tests.rs"]
mod tests;

/// A recipe for an ordinary certificate, owned by this preparation only.
/// Equations and derived facts cite fact positions instead of copying trees.
#[derive(Debug)]
enum Origin {
    Assumption(usize),
    SemanticAxiom(usize),
    PendingConjunct,
    Conjunction {
        parent: usize,
        conjunct: usize,
    },
    IntegerEquality {
        premise: usize,
    },
    Transport {
        premise: usize,
        equalities: Vec<usize>,
    },
}

/// One hypothesis or conjunct, with enough origin to reconstruct its proof.
#[derive(Debug)]
pub(crate) struct Fact {
    pub(crate) proposition: Proposition,
    origin: Origin,
    /// The index of a whole semantic axiom, cited by definition-bound rules.
    pub(crate) axiom: Option<usize>,
}

impl Fact {
    /// Expand only a selected derivation. The resulting owned tree is exactly
    /// what the unchanged kernel/publication path has always received, so its
    /// logical occurrence and depth budgets do not depend on physical sharing.
    pub(crate) fn proof(&self, facts: &[Fact]) -> ProofNode {
        let rule = match &self.origin {
            Origin::Assumption(index) => ProofRule::Assumption { index: *index },
            Origin::SemanticAxiom(index) => ProofRule::SemanticAxiom { index: *index },
            Origin::PendingConjunct => {
                unreachable!("conjunction construction is complete before proof selection")
            }
            Origin::Conjunction { parent, conjunct } => ProofRule::ConjunctionElimination {
                conjunction: Box::new(facts[*parent].proof(facts)),
                conjunct: *conjunct,
            },
            Origin::IntegerEquality { premise } => {
                let Proposition::Equal(_, right) = &facts[*premise].proposition else {
                    unreachable!("integer equality originates in a scalar equality");
                };
                ProofRule::EqualityTransitivity {
                    left_equals_middle: Box::new(facts[*premise].proof(facts)),
                    middle_equals_right: Box::new(ProofNode {
                        conclusion: Proposition::Equal(right.clone(), right.clone()),
                        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                    }),
                }
            }
            Origin::Transport {
                premise,
                equalities,
            } => ProofRule::ValueEqualityTransport {
                premise: Box::new(facts[*premise].proof(facts)),
                equalities: equalities
                    .iter()
                    .map(|&index| facts[index].proof(facts))
                    .collect(),
            },
        };
        ProofNode {
            conclusion: self.proposition.clone(),
            rule,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Known {
    pub(crate) facts: Vec<Fact>,
    /// Fact positions, in the original selected equation order.
    pub(crate) equations: Vec<usize>,
    /// The first checked definition of each value, retained from normalization
    /// so proof transport selects the same reachable equation family.
    pub(crate) definitions: IdTable<ValueId, Option<usize>>,
    /// `equations` followed by each compound integer equality an arrival
    /// supplied for a value nothing above defines, with `definitions`
    /// extended to cite them. Normalization never substitutes these: they
    /// stay premises for bound search. A goal the definitions alone cannot
    /// transport may still cite one, as `length == count - 0` from a match
    /// arm that read its window's length. Empty when there is none.
    pub(crate) arrival_equations: Vec<usize>,
    pub(crate) arrival_definitions: IdTable<ValueId, Option<usize>>,
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
    origin: Origin,
    axiom: Option<usize>,
    facts: &mut Vec<Fact>,
    conjuncts: &mut Vec<usize>,
) -> usize {
    let first = conjuncts.len();
    if let Proposition::Conjunction(parts) = proposition {
        for part in parts {
            let child = flatten(part, Origin::PendingConjunct, None, facts, conjuncts);
            conjuncts.push(child);
        }
    }
    let parent = facts.len();
    facts.push(Fact {
        proposition: proposition.clone(),
        origin,
        axiom,
    });
    // Keep the original children-before-parent search order. Parent references
    // point forward until this batch completes; no consumer sees pending
    // origins. One scratch stack serves all conjunctions, including nesting.
    for (conjunct, child) in conjuncts.drain(first..).enumerate() {
        facts[child].origin = Origin::Conjunction { parent, conjunct };
    }
    parent
}

/// Integer equality and equality of its embeddings have the same meaning.
/// Reuse the kernel's existing equality composition/normalization rule rather
/// than treating a callee's value equality as a new definition or assumption.
fn integer_equality(proposition: &Proposition) -> Option<Proposition> {
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
    Some(Proposition::IntegerMathEqual(first, second))
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
        values: &dyn ProofValues,
    ) -> Self {
        let mut facts = Vec::new();
        let mut conjuncts = Vec::new();
        for (index, assumption) in assumptions.iter().enumerate() {
            flatten(
                assumption,
                Origin::Assumption(index),
                None,
                &mut facts,
                &mut conjuncts,
            );
        }
        for (index, axiom) in axioms.iter().enumerate() {
            flatten(
                axiom,
                Origin::SemanticAxiom(index),
                Some(index),
                &mut facts,
                &mut conjuncts,
            );
        }
        let equations: Vec<_> = facts
            .iter()
            .enumerate()
            .filter(|(_, fact)| match &fact.proposition {
                Proposition::Equal(subject @ ScalarTerm::Value { .. }, definition) => {
                    match definition {
                        ScalarTerm::Boolean(_) => fact.axiom.is_some(),
                        ScalarTerm::Integer { .. } => true,
                        // A read with a checked initial-content equation:
                        // the available axiom ties this exact immutable value
                        // to its formal or producing call. Join scope is
                        // checked separately against the owning root.
                        ScalarTerm::Value { id, .. } => {
                            matches!(
                                subject.scalar_type(),
                                ScalarType::Boolean | ScalarType::Integer(_)
                            ) || (fact.axiom.is_some() && values.is_storage_content(*id))
                        }
                        // Exact operation definitions retain their selected
                        // integer meaning. A compound scalar equality learned
                        // from an arrival remains a proof premise for bounds.
                        _ => {
                            subject.scalar_type() == ScalarType::Boolean
                                || (fact.axiom.is_some()
                                    && matches!(subject.scalar_type(), ScalarType::Integer(_)))
                        }
                    }
                }
                _ => false,
            })
            .map(|(index, _)| index)
            .collect();
        let mut result = Self {
            facts,
            equations,
            definitions: IdTable::default(),
            arrival_equations: Vec::new(),
            arrival_definitions: IdTable::default(),
        };
        let mut defined = IdTable::default();
        for _ in 0..values.count() {
            defined.push(None);
        }
        for (index, &fact) in result.equations.iter().enumerate() {
            if let Proposition::Equal(ScalarTerm::Value { id, .. }, _) =
                &result.facts[fact].proposition
                && let Some(defined) = defined.get_mut(*id)
            {
                defined.get_or_insert(index);
            }
        }
        for index in 0..result.facts.len() {
            let _ = result.add_literal(index, &mut defined);
        }
        result.integer_equalities(0, &defined);
        result.normalize(context, &mut defined, &mut conjuncts);
        result.arrival_definitions(&defined);
        result.definitions = defined;
        result
    }

    /// Prepare `arrival_equations`: a checked premise `v == t`, `t` compound
    /// and not naming `v`, for an integer `v` with no definition. Each value
    /// keeps at most one definition, so transport through them terminates
    /// as it does through `equations`.
    fn arrival_definitions(&mut self, defined: &IdTable<ValueId, Option<usize>>) {
        let mut extended: Option<IdTable<ValueId, Option<usize>>> = None;
        for (index, fact) in self.facts.iter().enumerate() {
            let Proposition::Equal(
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(_),
                },
                definition,
            ) = &fact.proposition
            else {
                continue;
            };
            if fact.axiom.is_some()
                || matches!(
                    definition,
                    ScalarTerm::Value { .. } | ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_)
                )
                || defined.get(*id) != Some(&None)
            {
                continue;
            }
            let mut names_subject = false;
            definition.visit_value_ids(|value| {
                names_subject |= value == *id;
                true
            });
            if names_subject {
                continue;
            }
            let table = extended.get_or_insert_with(|| {
                self.arrival_equations.clone_from(&self.equations);
                defined.clone()
            });
            if let Some(slot) = table.get_mut(*id)
                && slot.is_none()
            {
                *slot = Some(self.arrival_equations.len());
                self.arrival_equations.push(index);
            }
        }
        if let Some(table) = extended {
            self.arrival_definitions = table;
        }
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
                && self.facts[self.equations[*definition]].proposition == fact.proposition
            {
                continue;
            }
            if let Some(proposition) = integer_equality(&fact.proposition) {
                self.facts.push(Fact {
                    proposition,
                    origin: Origin::IntegerEquality { premise: index },
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
        self.equations.push(index);
        Some(*id)
    }

    /// Cite the first definition of each value the predicate actually uses,
    /// following definitions transitively. Including every earlier derived
    /// equality would duplicate unrelated proof trees at every new step.
    pub(crate) fn transport_inputs(
        facts: &[Fact],
        equations: &[usize],
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
            if let Proposition::Equal(_, definition) = &facts[equations[*index]].proposition {
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
        conjuncts: &mut Vec<usize>,
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
            Self::transport_inputs(
                &self.facts,
                &self.equations,
                &fact.proposition,
                defined,
                &mut transport,
            );
            let Ok(transported) = check_value_equality_denotation(
                context,
                &fact.proposition,
                transport
                    .selected
                    .iter()
                    .map(|&index| &self.facts[self.equations[index]].proposition),
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
            let origin = Origin::Transport {
                premise: index,
                equalities: transport
                    .selected
                    .iter()
                    .map(|&index| self.equations[index])
                    .collect(),
            };
            let first = self.facts.len();
            flatten(&transported, origin, None, &mut self.facts, conjuncts);
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

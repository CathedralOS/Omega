//! Equality transport (carrying facts across proved equalities) at one
//! program point, shared by proof search and private-input description. A
//! taken Boolean comparison can expose another Boolean's value, including a
//! join parameter: `(b == true) == true` establishes `b == true` before a
//! predicate about `b` crosses a call.
//!
//! Every derived form keeps its transport evidence and original premises.
//! Definitions precede branch literals; a derived literal only fills an
//! undefined value. Dense value users revisit affected predicates,
//! and each value gains at most one definition. This computes no new domain,
//! authority or assumption; final obligation discharge still checks the
//! evidence with the kernel.

#[path = "known/normalization.rs"]
mod normalization;
#[path = "known/value_rows.rs"]
mod value_rows;
pub(crate) use normalization::{BranchPreparation, NormalizedPremises};
use std::borrow::Cow;
use std::cell::OnceCell;
use std::collections::VecDeque;
use std::sync::Arc;
pub(crate) use value_rows::{ValueFamily, ValueRows};

use arena::IdTable;
use proof_admission::{
    ProofNode, ProofRule, check_value_equality_denotation, check_value_equality_substitution,
};
use semantic_vocabulary::{
    IntegerCarrier, IntegerMathTerm, Proposition, PropositionContext, ScalarApplicationId,
    ScalarTerm, ScalarType, ValueId,
};
use terminal_check_tables::values::{ValueOrigin, ValueRow};
use terminal_psi::PrimitiveJudgment;

/// Declaration terms have logical inputs even when they have no executable
/// producer. The prover borrows their types from the owner supplying them.
pub(crate) trait ProofValues {
    fn count(&self) -> usize;
    fn identity(&self, position: usize) -> Option<ValueId>;
    fn scalar_type(&self, value: ValueId) -> Option<ScalarType>;

    // Declaration-only values have no storage origin.
    fn is_storage_content(&self, _: ValueId) -> bool {
        false
    }
}

impl ProofValues for IdTable<ValueId, ValueRow> {
    fn count(&self) -> usize {
        self.len()
    }

    fn identity(&self, position: usize) -> Option<ValueId> {
        self.identity_at(position)
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
    EqualitySymmetry {
        premise: usize,
    },
    Transport {
        premise: usize,
        equalities: Vec<usize>,
    },
}

/// One hypothesis or conjunct, with enough origin to reconstruct its proof.
#[derive(Debug)]
pub(crate) struct Fact<'source> {
    proposition: Cow<'source, Proposition>,
    origin: Origin,
    /// Conjuncts in this scope may cite the same proof of their whole premise.
    conjunction_proof: OnceCell<Arc<ProofNode>>,
    /// The index of a whole semantic axiom, cited by definition-bound rules.
    pub(crate) axiom: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DisjunctionOrigin {
    Assumption { index: usize, conjuncts: Vec<usize> },
    SemanticAxiom { index: usize, conjuncts: Vec<usize> },
}

/// Work owned by one fact-tree construction, never shared implicitly.
#[derive(Debug, Default)]
pub(crate) struct FactProofWork {
    #[cfg(test)]
    pub(crate) nodes: usize,
}

impl FactProofWork {
    fn record_node(&mut self) {
        #[cfg(test)]
        {
            self.nodes = self.nodes.saturating_add(1);
        }
    }
}

impl Fact<'_> {
    pub(crate) fn proposition(&self) -> &Proposition {
        &self.proposition
    }

    /// The same premise can have several transported disjunctions. A branch
    /// already selects one alternative of that premise, regardless of its
    /// current value names. Authored sibling conjuncts remain separate cuts.
    pub(crate) fn disjunction_origin(&self, facts: &[Fact]) -> Option<DisjunctionOrigin> {
        let mut selected = self;
        let mut conjuncts = Vec::new();
        let mut crossed_conjunction = false;
        for _ in 0..=facts.len() {
            match &selected.origin {
                Origin::Assumption(index) => {
                    conjuncts.reverse();
                    return Some(DisjunctionOrigin::Assumption {
                        index: *index,
                        conjuncts,
                    });
                }
                Origin::SemanticAxiom(index) => {
                    conjuncts.reverse();
                    return Some(DisjunctionOrigin::SemanticAxiom {
                        index: *index,
                        conjuncts,
                    });
                }
                Origin::Conjunction { parent, conjunct } => {
                    conjuncts.push(*conjunct);
                    crossed_conjunction = true;
                    selected = facts.get(*parent)?;
                }
                Origin::Transport { premise, .. } => {
                    let original = facts.get(*premise)?;
                    // Denotation sorts and flattens conjunctions. A child of
                    // the transformed conjunction has no authored ordinal.
                    if crossed_conjunction
                        || !matches!(selected.proposition(), Proposition::Disjunction(_))
                        || !matches!(original.proposition(), Proposition::Disjunction(_))
                    {
                        return None;
                    }
                    selected = original;
                }
                _ => return None,
            }
        }
        None
    }

    /// Expand a selected derivation. The checker visits the parent for every use of a conjunct.
    pub(crate) fn proof(&self, facts: &[Fact]) -> ProofNode {
        self.proof_with_work(facts, &mut FactProofWork::default())
    }

    pub(crate) fn proof_with_work(&self, facts: &[Fact], work: &mut FactProofWork) -> ProofNode {
        let rule = match &self.origin {
            Origin::Assumption(index) => ProofRule::Assumption { index: *index },
            Origin::SemanticAxiom(index) => ProofRule::SemanticAxiom { index: *index },
            Origin::PendingConjunct => {
                unreachable!("conjunction construction is complete before proof selection")
            }
            Origin::Conjunction { parent, conjunct } => ProofRule::ConjunctionElimination {
                conjunction: facts[*parent]
                    .conjunction_proof
                    .get_or_init(|| Arc::new(facts[*parent].proof_with_work(facts, work)))
                    .clone(),
                conjunct: *conjunct,
            },
            Origin::IntegerEquality { premise } => {
                let Proposition::Equal(_, right) = facts[*premise].proposition() else {
                    unreachable!("integer equality originates in a scalar equality");
                };
                work.record_node();
                ProofRule::EqualityTransitivity {
                    left_equals_middle: Box::new(facts[*premise].proof_with_work(facts, work)),
                    middle_equals_right: Box::new(ProofNode {
                        conclusion: Proposition::Equal(right.clone(), right.clone()),
                        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                    }),
                }
            }
            Origin::EqualitySymmetry { premise } => ProofRule::EqualitySymmetry {
                equality: Box::new(facts[*premise].proof_with_work(facts, work)),
            },
            Origin::Transport {
                premise,
                equalities,
            } => ProofRule::ValueEqualityTransport {
                premise: Box::new(facts[*premise].proof_with_work(facts, work)),
                equalities: equalities
                    .iter()
                    .map(|&index| facts[index].proof_with_work(facts, work))
                    .collect(),
            },
        };
        work.record_node();
        ProofNode {
            conclusion: self.proposition().clone(),
            rule,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Known<'source> {
    #[cfg(test)]
    pub(crate) normalization_checks: usize,
    pub(crate) facts: Vec<Fact<'source>>,
    /// Fact positions, in the original selected equation order.
    pub(crate) equations: Vec<usize>,
    /// The first checked definition of each value, retained from normalization
    /// so proof transport selects the same reachable equation family.
    pub(crate) definitions: ValueRows<Option<usize>>,
    /// `equations` followed by each compound integer equality an arrival
    /// supplied for a value nothing above defines, with `definitions`
    /// extended to cite them. Normalization never substitutes these: they
    /// stay premises for bound search. A goal the definitions alone cannot
    /// transport may still cite one, as `length == count - 0` from a match
    /// arm that read its window's length. Empty when there is none.
    pub(crate) arrival_equations: Vec<usize>,
    pub(crate) arrival_definitions: ValueRows<Option<usize>>,
}

/// One context's reusable equation traversal. Queries clear only the preceding
/// reachable family, rather than allocating and zeroing the full roster again.
pub(crate) struct TransportInputs {
    used: Vec<bool>,
    pub(crate) selected: Vec<usize>,
    pending: Vec<DefinitionSubject>,
    applications: Vec<(ScalarApplicationId, ScalarType, usize)>,
    indexed_equations: usize,
}

#[derive(Clone, Copy)]
enum DefinitionSubject {
    Value(ValueId),
    Application(ScalarApplicationId, ScalarType),
}

impl DefinitionSubject {
    fn of(term: &ScalarTerm) -> Option<Self> {
        match *term {
            ScalarTerm::Value { id, .. } => Some(Self::Value(id)),
            ScalarTerm::Application {
                application,
                scalar_type,
            } => Some(Self::Application(application, scalar_type)),
            _ => None,
        }
    }
}

impl TransportInputs {
    pub(crate) fn new(equations: usize) -> Self {
        Self {
            used: vec![false; equations],
            selected: Vec::new(),
            pending: Vec::new(),
            applications: Vec::new(),
            indexed_equations: 0,
        }
    }
}

/// Premise rows borrow the input tree. A transport result has a shorter lifetime
/// and supplies owned propositions for its new derivation.
fn flatten<'input, 'source>(
    proposition: &'input Proposition,
    retain: impl Fn(&'input Proposition) -> Cow<'source, Proposition> + Copy,
    origin: Origin,
    axiom: Option<usize>,
    facts: &mut Vec<Fact<'source>>,
    conjuncts: &mut Vec<usize>,
) -> usize {
    let first = conjuncts.len();
    if let Proposition::Conjunction(parts) = proposition {
        for part in parts {
            let child = flatten(part, retain, Origin::PendingConjunct, None, facts, conjuncts);
            conjuncts.push(child);
        }
    }
    let parent = facts.len();
    facts.push(Fact {
        conjunction_proof: OnceCell::new(),
        proposition: retain(proposition),
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

impl<'source> Known<'source> {
    pub(crate) fn new(
        context: &PropositionContext,
        assumptions: impl IntoIterator<Item = &'source Proposition>,
        axioms: impl IntoIterator<Item = &'source Proposition>,
        values: &dyn ProofValues,
    ) -> Self {
        Self::prepare(context, assumptions.into_iter(), axioms, values, None)
    }

    pub(crate) fn prepare(
        context: &PropositionContext,
        assumptions: impl Iterator<Item = &'source Proposition>,
        axioms: impl IntoIterator<Item = &'source Proposition>,
        values: &dyn ProofValues,
        mut branch: Option<&mut BranchPreparation<'_>>,
    ) -> Self {
        let mut facts = Vec::new();
        let mut conjuncts = Vec::new();
        for (index, assumption) in assumptions.enumerate() {
            flatten(
                assumption,
                Cow::Borrowed,
                Origin::Assumption(index),
                None,
                &mut facts,
                &mut conjuncts,
            );
        }
        let assumption_count = facts.len();
        for (index, axiom) in axioms.into_iter().enumerate() {
            flatten(
                axiom,
                Cow::Borrowed,
                Origin::SemanticAxiom(index),
                Some(index),
                &mut facts,
                &mut conjuncts,
            );
        }
        let declared_count = facts.len();
        // A supplied equality can name the application on either side.
        // Orient its proved symmetry for substitution; operation equations
        // keep their original read-to-application direction.
        for index in 0..facts.len() {
            let fact = &facts[index];
            let Proposition::Equal(left, right @ ScalarTerm::Application { .. }) =
                fact.proposition()
            else {
                continue;
            };
            if fact.axiom.is_some() || matches!(left, ScalarTerm::Application { .. }) {
                continue;
            }
            facts.push(Fact {
                conjunction_proof: OnceCell::new(),
                proposition: Cow::Owned(Proposition::Equal(right.clone(), left.clone())),
                origin: Origin::EqualitySymmetry { premise: index },
                axiom: None,
            });
        }
        let mut deferred_storage_aliases = Vec::new();
        let equations: Vec<_> = facts
            .iter()
            .enumerate()
            .filter(|(index, fact)| match fact.proposition() {
                Proposition::Equal(subject @ ScalarTerm::Value { .. }, definition) => {
                    match definition {
                        ScalarTerm::Boolean(_) => fact.axiom.is_some(),
                        ScalarTerm::Integer { .. } | ScalarTerm::IeeeFloatLiteral(_) => true,
                        // An exact scalar copy retains its carrier and value,
                        // including float bits. This is stored-value equality,
                        // not an IEEE comparison or a numeric conversion. The
                        // checked equation already names the immutable read;
                        // join scope is checked against the owning root.
                        ScalarTerm::Value { id: right, .. } => {
                            let ScalarTerm::Value { id: left, .. } = subject else {
                                unreachable!("equation subject matched ScalarTerm::Value");
                            };
                            if fact.axiom.is_none()
                                && left != right
                                && values.is_storage_content(*left)
                                && !values.is_storage_content(*right)
                            {
                                deferred_storage_aliases.push(*index);
                                false
                            } else {
                                true
                            }
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
                Proposition::Equal(subject @ ScalarTerm::Application { .. }, definition) => {
                    subject != definition
                }
                _ => false,
            })
            .map(|(index, _)| index)
            .collect();
        let mut result = Self {
            #[cfg(test)]
            normalization_checks: 0,
            facts,
            equations,
            definitions: ValueRows::default(),
            arrival_equations: Vec::new(),
            arrival_definitions: ValueRows::default(),
        };
        // Rows for the values these facts name; every other value of the
        // namespace reads as undefined (`ValueRows`).
        let mut defined =
            ValueFamily::of(values, result.facts.iter().map(|fact| fact.proposition()))
                .rows(None::<usize>);
        for (index, &fact) in result.equations.iter().enumerate() {
            let fact = &result.facts[fact];
            let Proposition::Equal(ScalarTerm::Value { id, .. }, _) = fact.proposition() else {
                continue;
            };
            let Some(definition) = defined.get_mut(*id) else {
                continue;
            };
            if let Some(previous) = *definition {
                let previous = &result.facts[result.equations[previous]];
                if fact.axiom.is_some()
                    && previous.axiom.is_none()
                    && fact.proposition() != previous.proposition()
                {
                    *definition = Some(index);
                }
            } else {
                *definition = Some(index);
            }
        }
        for index in 0..result.facts.len() {
            let _ = result.add_literal(index, &mut defined);
        }
        // Keep exact stored contents as the root when a join or contract
        // equates a storage snapshot to an undefined scalar value, while still
        // unfolding the snapshot when the scalar value has its own definition.
        for index in deferred_storage_aliases {
            let Proposition::Equal(
                left @ ScalarTerm::Value { id: left_id, .. },
                right @ ScalarTerm::Value { id: right_id, .. },
            ) = result.facts[index].proposition().clone()
            else {
                continue;
            };
            if defined.get(right_id) == Some(&None) {
                let symmetric = result.facts.len();
                result.facts.push(Fact {
                    conjunction_proof: OnceCell::new(),
                    proposition: Cow::Owned(Proposition::Equal(right, left)),
                    origin: Origin::EqualitySymmetry { premise: index },
                    axiom: None,
                });
                let equation = result.equations.len();
                result.equations.push(symmetric);
                if let Some(slot) = defined.get_mut(right_id) {
                    *slot = Some(equation);
                }
            } else if defined.get(left_id) == Some(&None) {
                let equation = result.equations.len();
                result.equations.push(index);
                if let Some(slot) = defined.get_mut(left_id) {
                    *slot = Some(equation);
                }
            }
        }
        result.integer_equalities(0, &defined);
        if !result.equations.is_empty()
            && let Some(scope) = branch.as_deref_mut()
        {
            scope.begin(assumption_count, declared_count);
        }
        result.normalize(context, &mut defined, &mut conjuncts, values, branch);
        result.arrival_definitions(&defined);
        result.definitions = defined;
        result
    }

    /// Prepare `arrival_equations`: a checked premise `v == t`, `t` compound
    /// and not naming `v`, for an integer `v` with no definition. Each value
    /// keeps at most one definition, so transport through them terminates
    /// as it does through `equations`.
    fn arrival_definitions(&mut self, defined: &ValueRows<Option<usize>>) {
        let mut extended: Option<ValueRows<Option<usize>>> = None;
        for (index, fact) in self.facts.iter().enumerate() {
            let Proposition::Equal(
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(_),
                },
                definition,
            ) = fact.proposition()
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
    fn integer_equalities(&mut self, first: usize, defined: &ValueRows<Option<usize>>) {
        let end = self.facts.len();
        for index in first..end {
            let fact = &self.facts[index];
            if let Proposition::Equal(ScalarTerm::Value { id, .. }, _) = fact.proposition()
                && let Some(Some(definition)) = defined.get(*id)
                && self.facts[self.equations[*definition]].proposition() == fact.proposition()
            {
                continue;
            }
            if let Some(proposition) = integer_equality(fact.proposition()) {
                self.facts.push(Fact {
                    conjunction_proof: OnceCell::new(),
                    proposition: Cow::Owned(proposition),
                    origin: Origin::IntegerEquality { premise: index },
                    axiom: None,
                });
            }
        }
    }

    fn add_literal(
        &mut self,
        index: usize,
        defined: &mut ValueRows<Option<usize>>,
    ) -> Option<ValueId> {
        let fact = &self.facts[index];
        if !is_branch_fact(fact.proposition()) {
            return None;
        }
        let Proposition::Equal(ScalarTerm::Value { id, .. }, _) = fact.proposition() else {
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

    /// Connect the remaining operands of a transported integer equality.
    /// They have no definitions left to unfold. Keep exact stored contents
    /// as the root when a copied or returned value is equal to them.
    fn add_integer_alias(
        &mut self,
        index: usize,
        defined: &mut ValueRows<Option<usize>>,
        values: &dyn ProofValues,
    ) -> Option<ValueId> {
        let Proposition::Equal(
            left @ ScalarTerm::Value {
                id: left_id,
                scalar_type: ScalarType::Integer(_),
            },
            right @ ScalarTerm::Value {
                id: right_id,
                scalar_type: ScalarType::Integer(_),
            },
        ) = self.facts[index].proposition()
        else {
            return None;
        };
        if left_id == right_id
            || defined.get(*left_id) != Some(&None)
            || defined.get(*right_id) != Some(&None)
        {
            return None;
        }
        let order = |id| (!values.is_storage_content(id), id);
        let (id, fact) = if order(*right_id) < order(*left_id) {
            (*left_id, index)
        } else {
            let id = *right_id;
            let fact = self.facts.len();
            self.facts.push(Fact {
                conjunction_proof: OnceCell::new(),
                proposition: Cow::Owned(Proposition::Equal(right.clone(), left.clone())),
                origin: Origin::EqualitySymmetry { premise: index },
                axiom: None,
            });
            (id, fact)
        };
        *defined.get_mut(id)? = Some(self.equations.len());
        self.equations.push(fact);
        Some(id)
    }

    /// Use the proof kernel's expansion budget for a shared computation
    /// graph. If it cannot be expanded, keep the original checked value.
    pub(crate) fn expand_value(
        &self,
        context: &PropositionContext,
        value: ValueId,
        scalar_type: ScalarType,
        transport: &mut TransportInputs,
    ) -> ScalarTerm {
        let actual = ScalarTerm::value(value, scalar_type);
        if self.definitions.get(value).is_none_or(Option::is_none) {
            return actual;
        }
        let identity = Proposition::Equal(actual.clone(), actual.clone());
        Self::transport_inputs(
            &self.facts,
            &self.equations,
            &identity,
            &self.definitions,
            transport,
        );
        match check_value_equality_substitution(
            context,
            &identity,
            transport
                .selected
                .iter()
                .map(|&index| self.facts[self.equations[index]].proposition()),
        ) {
            Ok(Proposition::Equal(definition, _)) => definition,
            _ => actual,
        }
    }

    /// Compare a read and the contents it denotes at this program point.
    pub(crate) fn same_denotation(
        &self,
        context: &PropositionContext,
        left: ScalarTerm,
        right: ScalarTerm,
        transport: &mut TransportInputs,
    ) -> bool {
        if left == right {
            return true;
        }
        let equality = Proposition::Equal(left, right);
        Self::transport_inputs(
            &self.facts,
            &self.equations,
            &equality,
            &self.definitions,
            transport,
        );
        match check_value_equality_denotation(
            context,
            &equality,
            transport
                .selected
                .iter()
                .map(|&index| self.facts[self.equations[index]].proposition()),
        ) {
            Ok(Proposition::Equal(left, right)) => left == right,
            Ok(Proposition::Truth) => true,
            _ => false,
        }
    }

    /// Cite the first definition of each value or application the predicate
    /// actually uses, following definitions transitively. Including every earlier derived
    /// equality would duplicate unrelated proof trees at every new step.
    pub(crate) fn transport_inputs(
        facts: &[Fact],
        equations: &[usize],
        proposition: &Proposition,
        defined: &ValueRows<Option<usize>>,
        scratch: &mut TransportInputs,
    ) {
        let TransportInputs {
            used,
            selected: inputs,
            pending,
            applications,
            indexed_equations,
        } = scratch;
        // Applications are immutable values with module-owned identities.
        // Index their checked equations once, including the carrier so a
        // different typed occurrence cannot borrow the same substitution.
        // Arrival equations extend the ordinary list. Reusing this lookup
        // for its shorter prefix must not expose an appended equation.
        let first_application = applications.len();
        for index in *indexed_equations..equations.len() {
            if let Proposition::Equal(
                ScalarTerm::Application {
                    application,
                    scalar_type,
                },
                _,
            ) = facts[equations[index]].proposition()
            {
                applications.push((*application, *scalar_type, index));
            }
        }
        *indexed_equations = (*indexed_equations).max(equations.len());
        if applications.len() != first_application {
            applications.sort_unstable();
            applications.dedup_by(|later, earlier| (later.0, later.1) == (earlier.0, earlier.1));
        }
        // Reset only the preceding traversal's selected definitions. New
        // equations append, so the invocation-owned marks only grow.
        for index in inputs.drain(..) {
            used[index] = false;
        }
        used.resize(equations.len(), false);
        proposition.visit_scalar_terms(|term| pending.extend(DefinitionSubject::of(term)));
        while let Some(subject) = pending.pop() {
            let index = match subject {
                DefinitionSubject::Value(value) => defined.get(value).copied().flatten(),
                DefinitionSubject::Application(application, scalar_type) => applications
                    .binary_search_by_key(&(application, scalar_type), |row| (row.0, row.1))
                    .ok()
                    .map(|position| applications[position].2)
                    .filter(|index| *index < equations.len()),
            };
            let Some(index) = index else { continue };
            if std::mem::replace(&mut used[index], true) {
                continue;
            }
            inputs.push(index);
            if let Proposition::Equal(_, definition) = facts[equations[index]].proposition() {
                definition.visit_terms(|term| pending.extend(DefinitionSubject::of(term)));
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
        defined: &mut ValueRows<Option<usize>>,
        conjuncts: &mut Vec<usize>,
        values: &dyn ProofValues,
        mut branch: Option<&mut BranchPreparation<'_>>,
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
        let mut users = defined.like(Vec::new());
        let mut transport = TransportInputs::new(self.equations.len());
        while let Some(index) = pending.pop_front() {
            queued[index] = false;
            let fact = &self.facts[index];
            Self::transport_inputs(
                &self.facts,
                &self.equations,
                fact.proposition(),
                defined,
                &mut transport,
            );
            let inherited = branch.as_ref().and_then(|scope| {
                scope.inherited(index, &self.facts, &self.equations, &transport.selected)
            });
            let transported = match inherited {
                Some(result) => Cow::Borrowed(result),
                None => {
                    #[cfg(test)]
                    {
                        self.normalization_checks += 1;
                    }
                    let Ok(result) = check_value_equality_denotation(
                        context,
                        fact.proposition(),
                        transport
                            .selected
                            .iter()
                            .map(|&i| self.facts[self.equations[i]].proposition()),
                    ) else {
                        continue;
                    };
                    Cow::Owned(result)
                }
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
            if transported.as_ref() == fact.proposition() {
                if let Some(scope) = branch.as_deref_mut() {
                    scope.record(index, index, &self.equations, &transport.selected);
                }
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
            let result = flatten(
                &transported,
                |derived| Cow::Owned(derived.clone()),
                origin,
                None,
                &mut self.facts,
                conjuncts,
            );
            if let Some(scope) = branch.as_deref_mut() {
                scope.record(index, result, &self.equations, &transport.selected);
            }
            self.integer_equalities(first, defined);
            for derived in first..self.facts.len() {
                if let Some(value) = self
                    .add_literal(derived, defined)
                    .or_else(|| self.add_integer_alias(derived, defined, values))
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

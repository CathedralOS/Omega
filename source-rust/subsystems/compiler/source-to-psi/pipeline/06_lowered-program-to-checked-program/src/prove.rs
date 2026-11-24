/*
The one prover. It builds a certificate for a goal from the hypotheses it is
given, in the kernel's own rules, and decides nothing itself: every node it
emits is re-checked by `verify_obligation`, and a certificate the kernel
refuses is `Unproved`.

The reasoning is interval bounds carried as proof. Each integer value gets a
best upper and lower literal bound, each with a node concluding
`v <= U` or `L <= v`:

- the carrier bound of its type (`IntegerCarrierBound`);
- a hypothesis that bounds it directly, or relates it to another value whose
  bound is then chained (`<=` transitivity, weakening, discreteness,
  symmetry, conjunction elimination);
- its own result equation when it is an exact add or subtract, mapped from
  its operands' bounds (`IntegerExactAddDefinitionBound` and the subtract
  twin);
- its own result equation when it is an exact multiply: the bound on the
  mathematical product (`IntegerAffineBound`) carried through the equation
  (`ValueEqualityTransport`) and restated in the carrier
  (`IntegerOrderSubstitution`);
- its own result equation when it is a divide, remainder, shift, bitwise and
  or bitwise or with one literal operand: a one-step endpoint chain the
  kernel replays (`ranges`);
- its own result equation when it is an exact left shift by a value that no
  earlier equation fixes: the bound on the mathematical shift from the
  value's bound and what any hypothesis says of the count, carried onto the
  output like a product's.

Goals decompose structurally (conjunction, disjunction, antisymmetry for
equality). A comparison closes when the left side's bound and the right
side's bound are ordered by a closed literal relation. An arithmetic safety
goal (`a + b <= MAX`) closes through `IntegerAffineBound`, whose image of the
operands' bounds the kernel computes (`map_integer_affine_bound`); this
module only asks for that image so it can state the node's conclusion.

Hypotheses arrive as two lists, exactly as the kernel receives them:
assumptions (entry `requires`, callee `ensures`) and semantic axioms
(operation result equations). Nothing here reads a name or text.
*/

use std::cmp::Ordering;
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;

use proof_admission::{
    IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule, check_integer_affine_witness,
    check_value_equality_denotation, compare_integer_math_terms, decide_primitive,
    lift_exact_integer_order, lower_integer_math_relation, map_integer_affine_bound,
};
use semantic_vocabulary::{
    IntegerCarrier, IntegerMathTerm, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};

#[path = "prove/counterexample.rs"]
mod counterexample;
#[path = "prove/float_integer.rs"]
mod float_integer;
#[path = "prove/float_meaning.rs"]
mod float_meaning;
#[path = "prove/ieee_float_order.rs"]
mod ieee_float_order;
#[path = "prove/mathematical_bounds.rs"]
mod mathematical_bounds;
#[path = "prove/ranges.rs"]
mod ranges;
#[path = "prove/reach.rs"]
mod reach;
#[cfg(test)]
#[path = "prove/tests.rs"]
mod tests;
#[path = "prove/widths.rs"]
mod widths;

use crate::checking::known::{Fact, ProofValues, ValueRows, is_branch_fact};

/// Why no certificate was built. Diagnostic only.
pub(crate) type Unproven = &'static str;

/// One obligation's case work, shared by all of its recursive searches.
/// Running out supplies no premise or partial certificate.
const CASE_BRANCH_BUDGET: usize = 64;

/// Build the candidate fact's tree with explicit invocation-owned work state.
/// Opposite-search proof work belongs to its own construction.
fn fact_proof_for_refutation(
    facts: &[Fact],
    index: usize,
    activity: &mut ProofSearchActivity,
) -> ProofNode {
    #[cfg(test)]
    {
        let mut work = crate::checking::known::FactProofWork::default();
        let proof = facts[index].proof_with_work(facts, &mut work);
        activity.refutation_fact_nodes = activity.refutation_fact_nodes.saturating_add(work.nodes);
        proof
    }
    #[cfg(not(test))]
    {
        let _ = activity;
        facts[index].proof(facts)
    }
}

/// Count the fact nodes built for a selected bound in tests.
fn fact_proof_for_literal_bound(
    facts: &[Fact],
    index: usize,
    activity: &mut ProofSearchActivity,
) -> ProofNode {
    #[cfg(test)]
    {
        let mut work = crate::checking::known::FactProofWork::default();
        let proof = facts[index].proof_with_work(facts, &mut work);
        activity.literal_bound_fact_nodes =
            activity.literal_bound_fact_nodes.saturating_add(work.nodes);
        proof
    }
    #[cfg(not(test))]
    {
        let _ = activity;
        facts[index].proof(facts)
    }
}

/// Classify a goal after proof search has failed. A covered form receives
/// `NoDerivation`. Other forms receive `NoRuleForGoalForm` and the checker
/// reports the obligation as incomplete. Proof search can still establish
/// such a goal, for example by combining known bounds on a sum's operands.
/// `fixed` identifies values whose equations supply a literal, so a closed
/// mathematical comparison can be evaluated after substitution.
fn covered(
    goal: &Proposition,
    fixed: &dyn Fn(ValueId) -> bool,
    width: &dyn Fn(ValueId) -> bool,
) -> bool {
    // The kernel also decides open expressions whose difference is constant.
    // Both comparison outcomes are covered: an unequal result is a refused
    // obligation, rather than an unsupported mathematical goal.
    if let Proposition::IntegerMathEqual(left, right)
    | Proposition::IntegerMathLessThan(left, right)
    | Proposition::IntegerMathLessOrEqual(left, right) = goal
        && matches!(compare_integer_math_terms(left, right), Ok(Some(_)))
    {
        return true;
    }
    match goal {
        Proposition::Truth | Proposition::Falsehood => true,
        Proposition::ScalarIeeeFloatComparison { .. }
        | Proposition::FloatMeaningEqual(_)
        | Proposition::FloatIntegerConversion(_)
        | Proposition::IeeeFloatIntegerExact { .. }
        | Proposition::IntegerIeeeFloatExact { .. } => true,
        Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
            parts.iter().all(|part| covered(part, fixed, width))
        }
        // The premise is only assumed; the conclusion is what is proved.
        Proposition::Implication { conclusion, .. } => covered(conclusion, fixed, width),
        Proposition::Equal(left, right) if byte_predicate_goal(left, right) => true,
        Proposition::LessOrEqual(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::Equal(left, right) => {
            matches!(left.scalar_type(), ScalarType::Integer(_))
                && reach::within_rules(left)
                && reach::within_rules(right)
        }
        // A comparison of two closed terms is the kernel's to evaluate
        // (`Prover::closed`), whatever its relation.
        Proposition::IntegerMathEqual(left, right) => {
            left == right || (closed_mathematical(left, fixed) && closed_mathematical(right, fixed))
        }
        Proposition::IntegerMathLessThan(left, right) => {
            closed_mathematical(left, fixed) && closed_mathematical(right, fixed)
        }
        Proposition::IntegerMathLessOrEqual(left, right) => {
            if closed_mathematical(left, fixed) && closed_mathematical(right, fixed) {
                return true;
            }
            // A range at a parameter width (`widths`).
            if widths::mentions_parameter_width(left, width)
                || widths::mentions_parameter_width(right, width)
            {
                return widths::covered(left, right, width);
            }
            // `Prover::arithmetic_interval` bounds one operation over values
            // and literals. A mathematical sum keeps each leaf's own carrier;
            // subtraction and multiplication retain their scalar signatures.
            // Deeper clause expressions are outside this covered subset.
            let leaf = |term: &IntegerMathTerm| match term {
                IntegerMathTerm::MathValue { source_type, .. } => Some(Some(*source_type)),
                IntegerMathTerm::IntegerLiteral(_) => Some(None),
                _ => None,
            };
            let bounded = |term: &IntegerMathTerm| match term {
                IntegerMathTerm::Add(first, second) => {
                    leaf(first).is_some() && leaf(second).is_some()
                }
                IntegerMathTerm::Subtract(first, second)
                | IntegerMathTerm::Multiply(first, second) => match (leaf(first), leaf(second)) {
                    (Some(first), Some(second)) => {
                        first.is_none() || second.is_none() || first == second
                    }
                    _ => false,
                },
                IntegerMathTerm::ShiftLeft { .. } | IntegerMathTerm::MathValue { .. } => true,
                IntegerMathTerm::IntegerLiteral(_) | IntegerMathTerm::Embed(_) => false,
            };
            match (left, right) {
                (term, IntegerMathTerm::IntegerLiteral(_))
                | (IntegerMathTerm::IntegerLiteral(_), term) => bounded(term),
                (IntegerMathTerm::MathValue { .. }, IntegerMathTerm::MathValue { .. }) => true,
                _ => false,
            }
        }
        _ => false,
    }
}

fn byte_predicate_goal(left: &ScalarTerm, right: &ScalarTerm) -> bool {
    matches!(
        (left, right),
        (
            ScalarTerm::ByteSequencePredicate { .. },
            ScalarTerm::Boolean(true)
        ) | (
            ScalarTerm::Boolean(true),
            ScalarTerm::ByteSequencePredicate { .. }
        )
    )
}

/// `covered` with no value fixed: the goal's form as written.
#[cfg(test)]
pub(crate) fn has_rules_for(goal: &Proposition) -> bool {
    covered(goal, &|_| false, &|_| false)
}

/// Whether `term` is literals, values that `fixed` says are literals, and
/// arithmetic over them.
fn closed_mathematical(term: &IntegerMathTerm, fixed: &dyn Fn(ValueId) -> bool) -> bool {
    match term {
        IntegerMathTerm::IntegerLiteral(_) => true,
        IntegerMathTerm::MathValue { value, .. } => fixed(*value),
        IntegerMathTerm::Embed(_) => false,
        IntegerMathTerm::Add(..)
        | IntegerMathTerm::Subtract(..)
        | IntegerMathTerm::Multiply(..)
        | IntegerMathTerm::ShiftLeft { .. } => term
            .operands()
            .into_iter()
            .all(|operand| closed_mathematical(operand, fixed)),
    }
}

/// How many literals `Prover::literal_bounds` tries as a bound that holds by
/// cases. Each is one or two goals; what lies beyond is simply not described.
const INTERVAL_CANDIDATES: usize = 12;

/// The literals `proposition` compares `subject` with, at any depth.
fn compared_literals(
    proposition: &Proposition,
    subject: &ScalarTerm,
    found: &mut Vec<IntegerValue>,
) {
    match proposition {
        Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
            for part in parts {
                compared_literals(part, subject, found);
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            compared_literals(premise, subject, found);
            compared_literals(conclusion, subject, found);
        }
        Proposition::LessOrEqual(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::Equal(left, right) => {
            for (value, literal) in [(left, right), (right, left)] {
                if value == subject
                    && let Some((_, literal)) = literal.integer_value()
                    && !found.contains(&literal)
                {
                    found.push(literal);
                }
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Upper,
    Lower,
}

impl Direction {
    const fn index(self) -> usize {
        match self {
            Self::Upper => 0,
            Self::Lower => 1,
        }
    }

    const fn opposite(self) -> Self {
        match self {
            Self::Upper => Self::Lower,
            Self::Lower => Self::Upper,
        }
    }

    const fn prefers(self, order: Option<Ordering>) -> bool {
        matches!(
            (self, order),
            (Self::Upper, Some(Ordering::Less)) | (Self::Lower, Some(Ordering::Greater))
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum Arithmetic {
    Add,
    Subtract,
    Multiply,
}

/// A literal bound on one value and the node that proves it.
#[derive(Debug, Clone)]
struct Bound {
    literal: IntegerValue,
    proof: ProofNode,
}

struct ComparisonSide {
    literal: IntegerValue,
    /// Present for an expression. A value's proof stays in the bound cache
    /// until the comparison succeeds.
    expression_proof: Option<ProofNode>,
}

/// A failed offset translation keeps the original relation for its fallback.
enum OffsetTranslation {
    Translated(Bound),
    Original(ProofNode),
}

/// Endpoint checks finish before the incoming proof is consumed.
struct MathematicalOrderTranslation {
    addend: IntegerMathTerm,
    translated_conclusion: Proposition,
    left_substituted_conclusion: Proposition,
    goal: Proposition,
    left_equality: ProofNode,
    right_equality: ProofNode,
}

impl MathematicalOrderTranslation {
    fn apply(self, relation: ProofNode) -> ProofNode {
        let translated = node(
            self.translated_conclusion,
            ProofRule::IntegerMathOrderTranslation {
                relation: Box::new(relation),
                addend: self.addend,
            },
        );
        let after_left = node(
            self.left_substituted_conclusion,
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(translated),
                equality: Box::new(self.left_equality),
                endpoint: 0,
            },
        );
        node(
            self.goal,
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(after_left),
                equality: Box::new(self.right_equality),
                endpoint: 1,
            },
        )
    }
}

/// Keep the fact's origin until its endpoint can improve the bound. Expanding
/// a discarded candidate would copy its entire supporting proof for nothing.
#[derive(Clone, Copy)]
enum CompoundOrder {
    Relation { fact: usize, strict: bool },
    Equality { fact: usize },
    ReversedEquality { fact: usize },
}

impl CompoundOrder {
    fn strict(self) -> bool {
        matches!(self, Self::Relation { strict: true, .. })
    }
}

#[derive(Debug, Clone)]
enum Slot {
    Unknown,
    InProgress(Option<Bound>),
    Done(Option<Bound>),
}

/// Hash only terms whose existing derived hash cannot recurse. Proposition's
/// logical traversal is iterative, but scalar and mathematical terms still
/// derive recursive Hash. Other forms keep exact equality in one fallback
/// bucket. Equal propositions always make the same eligibility choice.
fn exact_fact_hash_eligible(proposition: &Proposition) -> bool {
    let leaf = |term: &ScalarTerm| {
        matches!(
            term,
            ScalarTerm::Value { .. }
                | ScalarTerm::Integer { .. }
                | ScalarTerm::Boolean(_)
                | ScalarTerm::IeeeFloatLiteral(_)
        )
    };
    match proposition {
        Proposition::Truth | Proposition::Falsehood => true,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => leaf(left) && leaf(right),
        _ => false,
    }
}

/// Match an implication by its conclusion before attempting to prove its
/// premise. Simple comparisons get their own keys. Other conclusions share
/// a bucket with predicates of the same kind. Exact matching still decides
/// which rows to try, in the order the premises supplied them.
struct ImplicationConclusions {
    hash_builder: RandomState,
    positions: Vec<(u64, usize)>,
}

impl ImplicationConclusions {
    fn new(facts: &[Fact]) -> Self {
        let mut index = Self {
            hash_builder: RandomState::new(),
            positions: Vec::new(),
        };
        for (ordinal, fact) in facts.iter().enumerate() {
            if let Proposition::Implication { conclusion, .. } = &fact.proposition {
                index.positions.push((index.key(conclusion), ordinal));
            }
        }
        index.positions.sort_unstable();
        index
    }

    fn key(&self, proposition: &Proposition) -> u64 {
        let endpoints = if exact_fact_hash_eligible(proposition) {
            match proposition {
                Proposition::Equal(left, right) => {
                    // Equality matches in either order. Only the matching
                    // rule below can decide whether the terms are equal.
                    let mut keys = [
                        self.hash_builder.hash_one(left),
                        self.hash_builder.hash_one(right),
                    ];
                    keys.sort_unstable();
                    Some(keys)
                }
                _ => Some([self.hash_builder.hash_one(proposition), 0]),
            }
        } else {
            None
        };
        self.hash_builder
            .hash_one((std::mem::discriminant(proposition), endpoints))
    }

    fn candidates(&self, goal: &Proposition) -> std::ops::Range<usize> {
        if self.positions.is_empty() {
            return 0..0;
        }
        let key = self.key(goal);
        let start = self.positions.partition_point(|&(stored, _)| stored < key);
        let end = self.positions.partition_point(|&(stored, _)| stored <= key);
        start..end
    }
}

/// Find an equal fact without scanning unrelated predicates for every goal.
/// Facts are immutable after Known finishes; each prover owns its own index.
/// Hashes only choose candidates: existing equality decides them, in original
/// ordinal order even when hashes collide. Store no cloned propositions or
/// answers that could cross a branch's premise context.
struct ExactFacts<S = RandomState> {
    hash_builder: S,
    positions: Vec<(Option<u64>, usize)>,
}

impl ExactFacts {
    fn new(facts: &[Fact]) -> Self {
        Self::with_hasher(facts, RandomState::new())
    }
}

impl<S: BuildHasher> ExactFacts<S> {
    fn with_hasher(facts: &[Fact], hash_builder: S) -> Self {
        let mut result = Self {
            hash_builder,
            positions: Vec::with_capacity(facts.len()),
        };
        for (ordinal, fact) in facts.iter().enumerate() {
            let key = result.key(&fact.proposition);
            result.positions.push((key, ordinal));
        }
        // Both keys are primitive integers; never order a recursive proposition.
        result.positions.sort_unstable();
        result
    }

    fn key(&self, proposition: &Proposition) -> Option<u64> {
        if exact_fact_hash_eligible(proposition) {
            Some(self.hash_builder.hash_one(proposition))
        } else {
            None
        }
    }

    fn find(
        &self,
        facts: &[Fact],
        goal: &Proposition,
        activity: &mut ProofSearchActivity,
    ) -> Option<usize> {
        let key = self.key(goal);
        let start = self.positions.partition_point(|(stored, _)| *stored < key);
        let end = self.positions.partition_point(|(stored, _)| *stored <= key);
        #[cfg(test)]
        let mut comparisons = 0usize;
        let found = self.positions[start..end].iter().find_map(|&(_, ordinal)| {
            #[cfg(test)]
            {
                comparisons = comparisons.saturating_add(1);
            }
            (&facts[ordinal].proposition == goal).then_some(ordinal)
        });
        #[cfg(test)]
        {
            let work = &mut activity.exact_fact_work;
            work.lookups = work.lookups.saturating_add(1);
            work.comparisons = work.comparisons.saturating_add(comparisons);
            if found.is_none() {
                work.misses = work.misses.saturating_add(1);
                work.miss_comparisons = work.miss_comparisons.saturating_add(comparisons);
            }
            if key.is_none() {
                work.fallback_lookups = work.fallback_lookups.saturating_add(1);
                work.fallback_comparisons = work.fallback_comparisons.saturating_add(comparisons);
                if found.is_none() {
                    work.fallback_misses = work.fallback_misses.saturating_add(1);
                    work.fallback_miss_comparisons =
                        work.fallback_miss_comparisons.saturating_add(comparisons);
                }
            }
        }
        #[cfg(not(test))]
        let _ = activity;
        found
    }
}

#[cfg(test)]
#[derive(Debug, Default)]
struct ExactFactWork {
    prepared_facts: usize,
    prepared_fallback_facts: usize,
    lookups: usize,
    comparisons: usize,
    misses: usize,
    miss_comparisons: usize,
    fallback_lookups: usize,
    fallback_comparisons: usize,
    fallback_misses: usize,
    fallback_miss_comparisons: usize,
}

#[cfg(test)]
impl ExactFactWork {
    fn merge(&mut self, other: Self) {
        self.prepared_facts = self.prepared_facts.saturating_add(other.prepared_facts);
        self.prepared_fallback_facts = self
            .prepared_fallback_facts
            .saturating_add(other.prepared_fallback_facts);
        self.lookups = self.lookups.saturating_add(other.lookups);
        self.comparisons = self.comparisons.saturating_add(other.comparisons);
        self.misses = self.misses.saturating_add(other.misses);
        self.miss_comparisons = self.miss_comparisons.saturating_add(other.miss_comparisons);
        self.fallback_lookups = self.fallback_lookups.saturating_add(other.fallback_lookups);
        self.fallback_comparisons = self
            .fallback_comparisons
            .saturating_add(other.fallback_comparisons);
        self.fallback_misses = self.fallback_misses.saturating_add(other.fallback_misses);
        self.fallback_miss_comparisons = self
            .fallback_miss_comparisons
            .saturating_add(other.fallback_miss_comparisons);
    }
}

/// Bound construction needs a fact naming the queried value. Keep its original
/// ordinal so both passes retain their first proof when endpoints tie. Other
/// searches, including contradiction, still read the complete fact context.
struct BoundFacts {
    by_value: ValueRows<Vec<usize>>,
    fallback: Vec<usize>,
}

#[derive(Default)]
struct BoundFactCursor {
    value_position: usize,
    fallback_position: usize,
}

impl BoundFacts {
    fn new(facts: &[Fact], declarations: &ValueRows<Option<usize>>) -> Self {
        let mut result = Self {
            by_value: declarations.like(Vec::new()),
            fallback: Vec::new(),
        };
        for (index, fact) in facts.iter().enumerate() {
            // These are the forms read by literal_bound_from_fact,
            // bound_from_fact and compound_bound_from_fact. A closed false
            // fact or a disjunction remains available to their own searches.
            if !matches!(
                fact.proposition,
                Proposition::LessOrEqual(..)
                    | Proposition::LessThan(..)
                    | Proposition::Equal(..)
                    | Proposition::IntegerMathEqual(..)
                    | Proposition::IntegerMathLessOrEqual(..)
            ) {
                continue;
            }
            let complete = fact.proposition.visit_value_ids(|value| {
                if let Some(indices) = result.by_value.get_mut(value)
                    && indices.last() != Some(&index)
                {
                    indices.push(index);
                }
            });
            if !complete {
                // Opaque terms need the ordinary matcher even when the
                // visitor exposed no subject. Partial visits may also have
                // indexed this fact; the ordinal merge removes that duplicate.
                result.fallback.push(index);
            }
        }
        result
    }

    fn next(&self, value: ValueId, cursor: &mut BoundFactCursor) -> Option<usize> {
        let local = self
            .by_value
            .get(value)
            .and_then(|indices| indices.get(cursor.value_position))
            .copied();
        let fallback = self.fallback.get(cursor.fallback_position).copied();
        match (local, fallback) {
            (Some(local), Some(fallback)) => {
                if local <= fallback {
                    cursor.value_position += 1;
                }
                if fallback <= local {
                    cursor.fallback_position += 1;
                }
                Some(local.min(fallback))
            }
            (Some(index), None) => {
                cursor.value_position += 1;
                Some(index)
            }
            (None, Some(index)) => {
                cursor.fallback_position += 1;
                Some(index)
            }
            (None, None) => None,
        }
    }
}

/// Searches started during one explicit proof invocation. Callers retain the
/// activity across certificate reuse to detect any additional search.
#[derive(Debug, Default)]
pub struct ProofSearchActivity {
    #[cfg(test)]
    normalization_checks: usize,
    started: usize,
    #[cfg(test)]
    sign_queries: usize,
    #[cfg(test)]
    contradiction_searches: usize,
    #[cfg(test)]
    in_progress_bound_reuses: usize,
    #[cfg(test)]
    disjunction_materializations: usize,
    /// Owned certificate copies returned by `bound`, excluding scalar-only queries.
    #[cfg(test)]
    bound_certificate_copies: usize,
    /// Nodes emitted for the candidate fact, excluding the opposite search.
    #[cfg(test)]
    refutation_fact_nodes: usize,
    /// Fact nodes materialized for compound order candidates.
    #[cfg(test)]
    compound_order_fact_nodes: usize,
    /// Fact nodes expanded for direct literal-bound candidates.
    #[cfg(test)]
    literal_bound_fact_nodes: usize,
    #[cfg(test)]
    bound_value_searches: usize,
    #[cfg(test)]
    bound_fact_visits: usize,
    #[cfg(test)]
    exact_fact_work: ExactFactWork,
    /// Value rows each prepared context allocated (`ValueRows`).
    #[cfg(test)]
    value_rows: usize,
}

impl ProofSearchActivity {
    #[cfg(any(test, feature = "test-support"))]
    pub fn started(&self) -> usize {
        self.started
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.started = self.started.saturating_add(other.started);
        #[cfg(test)]
        {
            self.sign_queries = self.sign_queries.saturating_add(other.sign_queries);
            self.contradiction_searches = self
                .contradiction_searches
                .saturating_add(other.contradiction_searches);
            self.in_progress_bound_reuses = self
                .in_progress_bound_reuses
                .saturating_add(other.in_progress_bound_reuses);
            self.disjunction_materializations = self
                .disjunction_materializations
                .saturating_add(other.disjunction_materializations);
            self.bound_certificate_copies = self
                .bound_certificate_copies
                .saturating_add(other.bound_certificate_copies);
            self.refutation_fact_nodes = self
                .refutation_fact_nodes
                .saturating_add(other.refutation_fact_nodes);
            self.compound_order_fact_nodes = self
                .compound_order_fact_nodes
                .saturating_add(other.compound_order_fact_nodes);
            self.literal_bound_fact_nodes = self
                .literal_bound_fact_nodes
                .saturating_add(other.literal_bound_fact_nodes);
            self.bound_value_searches = self
                .bound_value_searches
                .saturating_add(other.bound_value_searches);
            self.bound_fact_visits = self
                .bound_fact_visits
                .saturating_add(other.bound_fact_visits);
            self.exact_fact_work.merge(other.exact_fact_work);
            self.value_rows = self.value_rows.saturating_add(other.value_rows);
            self.normalization_checks = self
                .normalization_checks
                .saturating_add(other.normalization_checks);
        }
    }
}

#[derive(Debug)]
struct ActiveCut {
    proposition: Proposition,
    origin: Option<crate::checking::known::DisjunctionOrigin>,
}

#[derive(Clone)]
struct Assumptions<'a> {
    original: &'a [Proposition],
    local: Vec<&'a Proposition>,
}

impl Assumptions<'_> {
    fn len(&self) -> usize {
        self.original.len() + self.local.len()
    }

    fn iter(&self) -> impl Iterator<Item = &Proposition> {
        self.original.iter().chain(self.local.iter().copied())
    }
}

pub(crate) struct Prover<'a> {
    activity: &'a mut ProofSearchActivity,
    context: &'a PropositionContext,
    assumptions: Assumptions<'a>,
    normalized_premises: crate::checking::known::NormalizedPremises,
    axioms: &'a [Proposition],
    values: &'a dyn ProofValues,
    facts: Vec<Fact>,
    exact_facts: ExactFacts,
    /// Actual implication facts, prepared once for goal-directed elimination.
    implications: ImplicationConclusions,
    float_integer_equalities: float_integer::Equalities,
    /// Checked definitions for transport: Boolean values, literals, exact
    /// initial-content aliases, and semantic integer expressions retaining
    /// their selected arithmetic meaning, each citing its prepared fact.
    equations: Vec<usize>,
    definitions: ValueRows<Option<usize>>,
    /// `equations` extended by arrivals' compound equalities, which only
    /// `by_transport` cites after `equations` alone fails (`Known`).
    arrival_equations: Vec<usize>,
    arrival_definitions: ValueRows<Option<usize>>,
    transport: crate::checking::known::TransportInputs,
    /// `[upper, lower]` in the exact declared value namespace. A contract
    /// witness may begin at any real formal ID, not only at value one.
    bounds: ValueRows<[Slot; 2]>,
    /// Each branch indexes its own facts, including the assumption added on entry.
    bound_facts: BoundFacts,
    /// Whether a failed goal may split over a disjunctive fact. Guard
    /// contradiction search disables this to avoid re-entering itself.
    case_split: bool,
    /// Shared with each branch/implication and returned after its search.
    remaining_case_branches: usize,
    /// A branch already assumes one alternative of each cut on this path.
    /// Splitting that exact predicate again supplies no new information.
    active_cuts: Vec<ActiveCut>,
    /// An auxiliary sign query can revisit its parent through contradiction
    /// search. The same goal under these unchanged premises supplies no proof.
    active_goals: Vec<Proposition>,
    /// A direct Falsehood or unreachable query already searches these premises
    /// for a contradiction. Repeating that search for another conclusion adds
    /// no information and multiplies work by the order facts at each level.
    refuting: bool,
}

fn node(conclusion: Proposition, rule: ProofRule) -> ProofNode {
    ProofNode { conclusion, rule }
}

fn primitive(conclusion: Proposition, judgment: PrimitiveJudgment) -> ProofNode {
    node(conclusion, ProofRule::Primitive(judgment))
}

fn step(value: IntegerValue, increase: bool) -> Option<IntegerValue> {
    match (value, increase) {
        (IntegerValue::Signed(value), true) => value.checked_add(1).map(IntegerValue::Signed),
        (IntegerValue::Signed(value), false) => value.checked_sub(1).map(IntegerValue::Signed),
        (IntegerValue::Unsigned(value), true) => value.checked_add(1).map(IntegerValue::Unsigned),
        (IntegerValue::Unsigned(value), false) => value.checked_sub(1).map(IntegerValue::Unsigned),
    }
}

fn term_mentions(term: &ScalarTerm, target: &ScalarTerm) -> bool {
    term == target
        || term
            .operands()
            .into_iter()
            .any(|operand| term_mentions(operand, target))
}

/// When `term` is built from `subject` by exact fixed-carrier additions and
/// subtractions with closed integer offsets, returns `term - subject`.
fn exact_constant_offset(
    term: &ScalarTerm,
    subject: &ScalarTerm,
    integer: IntegerType,
) -> Option<i128> {
    if term == subject {
        return Some(0);
    }
    let as_i128 = |value: IntegerValue| match value {
        IntegerValue::Signed(v) => Some(v),
        IntegerValue::Unsigned(v) => i128::try_from(v).ok(),
    };
    match term {
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer => {
            if let Some((ty, val)) = right.integer_value()
                && ty == integer
            {
                return exact_constant_offset(left, subject, integer)?.checked_add(as_i128(val)?);
            }
            if let Some((ty, val)) = left.integer_value()
                && ty == integer
            {
                return exact_constant_offset(right, subject, integer)?.checked_add(as_i128(val)?);
            }
            None
        }
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } if *scalar_type == integer => {
            let (ty, val) = right.integer_value()?;
            if ty != integer {
                return None;
            }
            exact_constant_offset(left, subject, integer)?.checked_sub(as_i128(val)?)
        }
        _ => None,
    }
}

/// `value - delta` when representable in `integer`.
fn shift_integer_value(
    integer: IntegerType,
    value: IntegerValue,
    delta: i128,
) -> Option<IntegerValue> {
    let shifted = match value {
        IntegerValue::Signed(v) => IntegerValue::Signed(v.checked_sub(delta)?),
        IntegerValue::Unsigned(v) => {
            let result = if delta >= 0 {
                v.checked_sub(delta.unsigned_abs())?
            } else {
                v.checked_add(delta.unsigned_abs())?
            };
            IntegerValue::Unsigned(result)
        }
    };
    integer.admits(shifted).then_some(shifted)
}

fn sorted_math_equal(first: IntegerMathTerm, second: IntegerMathTerm) -> Proposition {
    if first <= second {
        Proposition::IntegerMathEqual(first, second)
    } else {
        Proposition::IntegerMathEqual(second, first)
    }
}

fn is_bare_scalar(term: &ScalarTerm) -> bool {
    matches!(term, ScalarTerm::Value { .. } | ScalarTerm::Integer { .. })
}

/// The two sides of a `<=` or `<`, and whether it is strict.
fn order(proposition: &Proposition) -> Option<(bool, &ScalarTerm, &ScalarTerm)> {
    match proposition {
        Proposition::LessOrEqual(left, right) => Some((false, left, right)),
        Proposition::LessThan(left, right) => Some((true, left, right)),
        _ => None,
    }
}

/// This rule needs one side to evaluate to an integer constant. Skip other
/// orders before searching for the opposite or expanding their evidence;
/// neither can produce a certificate through this rule.
fn order_with_closed_endpoint(
    proposition: &Proposition,
) -> Option<(bool, &ScalarTerm, &ScalarTerm)> {
    let order @ (_, left, right) = order(proposition)?;
    (left.integer_value().is_some() || right.integer_value().is_some()).then_some(order)
}

/// Chains `a R b` and `b R' c` into `a R'' c`, strict when either step is.
fn chain(first: ProofNode, second: ProofNode) -> Result<ProofNode, Unproven> {
    let (Some((first_strict, left, _)), Some((second_strict, _, right))) =
        (order(&first.conclusion), order(&second.conclusion))
    else {
        return Err("chain over a step that is not an order");
    };
    let (left, right) = (left.clone(), right.clone());
    Ok(if first_strict || second_strict {
        node(
            Proposition::LessThan(left, right),
            ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle: Box::new(first),
                middle_to_right: Box::new(second),
            },
        )
    } else {
        node(
            Proposition::LessOrEqual(left, right),
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(first),
                middle_less_or_equal_right: Box::new(second),
            },
        )
    })
}

/// `low + addend <= high + addend` from `low <= high`.
fn translate(proof: ProofNode, addend: IntegerMathTerm) -> Option<ProofNode> {
    let Proposition::IntegerMathLessOrEqual(low, high) = &proof.conclusion else {
        return None;
    };
    let conclusion = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(Box::new(low.clone()), Box::new(addend.clone())),
        IntegerMathTerm::Add(Box::new(high.clone()), Box::new(addend.clone())),
    );
    Some(node(
        conclusion,
        ProofRule::IntegerMathOrderTranslation {
            relation: Box::new(proof),
            addend,
        },
    ))
}

/// The two ends of a proved mathematical `<=`.
fn ends(proof: &ProofNode) -> Option<(&IntegerMathTerm, &IntegerMathTerm)> {
    match &proof.conclusion {
        Proposition::IntegerMathLessOrEqual(low, high) => Some((low, high)),
        _ => None,
    }
}

/// `a <= c` from `a <= b` and `b <= c`, both mathematical.
fn chain_math(first: ProofNode, second: ProofNode) -> Option<ProofNode> {
    let (low, first_middle) = ends(&first)?;
    let (second_middle, high) = ends(&second)?;
    if first_middle != second_middle {
        return None;
    }
    let conclusion = Proposition::IntegerMathLessOrEqual(low.clone(), high.clone());
    Some(node(
        conclusion,
        ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(first),
            middle_less_or_equal_right: Box::new(second),
        },
    ))
}

/// An equality or strict order implies the corresponding non-strict order.
fn weaken(relation: ProofNode) -> Result<ProofNode, Unproven> {
    let (Proposition::Equal(left, right) | Proposition::LessThan(left, right)) =
        &relation.conclusion
    else {
        return Err("weakening requires an equality or strict order");
    };
    let conclusion = Proposition::LessOrEqual(left.clone(), right.clone());
    Ok(node(
        conclusion,
        ProofRule::IntegerOrderWeakening {
            relation: Box::new(relation),
        },
    ))
}

/// `a == b` to `b == a`.
fn flip(equality: ProofNode) -> Result<ProofNode, Unproven> {
    let Proposition::Equal(left, right) = &equality.conclusion else {
        return Err("symmetry of a non-equality");
    };
    let conclusion = Proposition::Equal(right.clone(), left.clone());
    Ok(node(
        conclusion,
        ProofRule::EqualitySymmetry {
            equality: Box::new(equality),
        },
    ))
}

/// `goal` from `absurd`, a proved proposition whose denotation is
/// `Falsehood` (a closed false relation, or `true == false`): `Falsehood and
/// goal` has that same denotation, so it holds, and so does `goal`
/// (`wiki/spec/proofs/classicality.md`, contradiction elimination). The
/// kernel recomputes both denotations and refuses when `absurd`'s is not
/// `Falsehood`.
fn ex_falso(absurd: ProofNode, goal: &Proposition) -> ProofNode {
    let both = node(
        Proposition::Conjunction(vec![Proposition::Falsehood, goal.clone()]),
        ProofRule::PredicateDenotation {
            premise: Box::new(absurd),
        },
    );
    node(
        goal.clone(),
        ProofRule::ConjunctionElimination {
            conjunction: both.into(),
            conjunct: 1,
        },
    )
}

impl<'a> Prover<'a> {
    pub(crate) fn new(
        context: &'a PropositionContext,
        assumptions: &'a [Proposition],
        axioms: &'a [Proposition],
        values: &'a dyn ProofValues,
        activity: &'a mut ProofSearchActivity,
    ) -> Self {
        Self::prepare(
            context,
            Assumptions {
                original: assumptions,
                local: Vec::new(),
            },
            axioms,
            values,
            activity,
            None,
        )
    }

    fn prepare(
        context: &'a PropositionContext,
        assumptions: Assumptions<'a>,
        axioms: &'a [Proposition],
        values: &'a dyn ProofValues,
        activity: &'a mut ProofSearchActivity,
        parent: Option<(&[Fact], &crate::checking::known::NormalizedPremises)>,
    ) -> Self {
        let mut branch = crate::checking::known::BranchPreparation::new(parent);
        activity.started = activity.started.saturating_add(1);
        let crate::checking::known::Known {
            #[cfg(test)]
            normalization_checks,
            facts,
            equations,
            definitions,
            arrival_equations,
            arrival_definitions,
        } = crate::checking::known::Known::prepare(
            context,
            assumptions.iter(),
            axioms,
            values,
            Some(&mut branch),
        );
        let implications = ImplicationConclusions::new(&facts);
        let float_integer_equalities = float_integer::Equalities::new(&facts);
        let transport = crate::checking::known::TransportInputs::new(equations.len());
        let exact_facts = ExactFacts::new(&facts);
        #[cfg(test)]
        {
            activity.normalization_checks += normalization_checks;
            activity.exact_fact_work.prepared_facts = activity
                .exact_fact_work
                .prepared_facts
                .saturating_add(facts.len());
            activity.exact_fact_work.prepared_fallback_facts = activity
                .exact_fact_work
                .prepared_fallback_facts
                .saturating_add(
                    exact_facts
                        .positions
                        .partition_point(|(key, _)| key.is_none()),
                );
        }
        let bound_facts = BoundFacts::new(&facts, &definitions);
        let bounds = definitions.like([Slot::Unknown, Slot::Unknown]);
        #[cfg(test)]
        {
            activity.value_rows = activity.value_rows.saturating_add(bounds.row_count());
        }
        Self {
            activity,
            context,
            assumptions,
            normalized_premises: branch.prepared,
            axioms,
            values,
            facts,
            exact_facts,
            implications,
            float_integer_equalities,
            equations,
            definitions,
            arrival_equations,
            arrival_definitions,
            transport,
            bounds,
            bound_facts,
            case_split: true,
            remaining_case_branches: CASE_BRANCH_BUDGET,
            active_cuts: Vec::new(),
            active_goals: Vec::new(),
            refuting: false,
        }
    }

    fn exact_fact(&mut self, goal: &Proposition) -> Option<usize> {
        self.exact_facts.find(&self.facts, goal, self.activity)
    }

    /// `goal` by `ValueEqualityTransport`: under the definitions the prover
    /// holds, its denotation is `Truth`, or is the denotation of a fact. The
    /// kernel recomputes both denotations from the same definitions. Only
    /// when those definitions do not decide it, an arrival's compound
    /// equality may define its otherwise undefined value too: a match arm
    /// that read `window.length` crosses as `length == count - 0`, which the
    /// window's later measurement `count - start` with `start == 0` meets
    /// only by transport.
    fn by_transport(&mut self, goal: &Proposition) -> Option<ProofNode> {
        self.transport_through(goal, false)
            .or_else(|| self.transport_through(goal, true))
    }

    fn transport_through(&mut self, goal: &Proposition, arrivals: bool) -> Option<ProofNode> {
        if arrivals && self.arrival_equations.is_empty() {
            return None;
        }
        let (equations, definitions) = if arrivals {
            (&self.arrival_equations, &self.arrival_definitions)
        } else {
            (&self.equations, &self.definitions)
        };
        if equations.is_empty() {
            return None;
        }
        crate::checking::known::Known::transport_inputs(
            &self.facts,
            equations,
            goal,
            definitions,
            &mut self.transport,
        );
        let target = check_value_equality_denotation(
            self.context,
            goal,
            self.transport
                .selected
                .iter()
                .map(|&index| &self.facts[equations[index]].proposition),
        )
        .ok()?;
        let equalities: Vec<ProofNode> = self
            .transport
            .selected
            .iter()
            .map(|&index| self.facts[equations[index]].proof(&self.facts))
            .collect();
        let premise = if target == Proposition::Truth {
            primitive(Proposition::Truth, PrimitiveJudgment::Truth)
        } else if let Some(index) = self.exact_fact(&target) {
            self.facts[index].proof(&self.facts)
        } else {
            // Result equations and qualification aliases can expose identical
            // open expressions. The kernel checks reflexivity without changing
            // the selected operation, including wrapping or saturating terms;
            // transport still checks every equation and the original goal.
            // The denotation leaves mathematical arithmetic as written
            // (fact_validity.md: "Open integer relations, mathematical
            // arithmetic, IEEE relations, and storage observations remain
            // unchanged"), so a mathematical comparison whose values are
            // all fixed to literals arrives here closed and not yet
            // decided. The kernel evaluates it.
            self.reflexive(target.clone())
                .or_else(|| self.closed(target.clone()))
                .or_else(|| self.sum_order(&target))
                .or_else(|| self.multiple_remainder(&target))
                // An exact copied value may expose an open relation. Reuse
                // ordinary proof rules only when its denotation has changed.
                .or_else(|| {
                    (target != *goal)
                        .then(|| self.prove_direct_subgoal(&target).ok())
                        .flatten()
                })?
        };
        Some(node(
            goal.clone(),
            ProofRule::ValueEqualityTransport {
                premise: Box::new(premise),
                equalities,
            },
        ))
    }

    /// Reuse normalization's exact definition closure. Unrelated arithmetic
    /// must not consume this goal's transport budget or duplicate proof trees.
    fn select_transport_inputs(&mut self, proposition: &Proposition) {
        crate::checking::known::Known::transport_inputs(
            &self.facts,
            &self.equations,
            proposition,
            &self.definitions,
            &mut self.transport,
        );
    }

    fn transported_goal(&mut self, goal: &Proposition) -> Option<Proposition> {
        self.select_transport_inputs(goal);
        check_value_equality_denotation(
            self.context,
            goal,
            self.transport
                .selected
                .iter()
                .map(|&index| &self.facts[self.equations[index]].proposition),
        )
        .ok()
    }

    /// Whether the rules cover `goal`'s form (`covered`) at this prover's
    /// program point: a value one of its equations fixes to a literal is that
    /// literal.
    pub(crate) fn has_rules_for(&mut self, goal: &Proposition) -> bool {
        let fixed = |facts: &[Fact], equations: &[usize], value| {
            equations.iter().any(|&index| {
                let equation = &facts[index].proposition;
                matches!(equation,
                    Proposition::Equal(ScalarTerm::Value { id, .. }, ScalarTerm::Integer { .. })
                    if *id == value)
            })
        };
        let binders = self.width_binders();
        let width = |value| binders.contains(&value);
        if covered(
            goal,
            &|value| fixed(&self.facts, &self.equations, value),
            &width,
        ) {
            return true;
        }
        // Use the same exact definitions as certificate construction. A
        // checked update can expose `entry + 1 == entry + 2`: the kernel can
        // decide that comparison even though its original result was open.
        self.transported_goal(goal).is_some_and(|goal| {
            covered(
                &goal,
                &|value| fixed(&self.facts, &self.equations, value),
                &width,
            )
        })
    }

    fn closed(&self, proposition: Proposition) -> Option<ProofNode> {
        if matches!(proposition, Proposition::IntegerIeeeFloatExact { .. }) {
            return proof_admission::check_integer_float_exact(self.context, &proposition)
                .ok()
                .map(|()| node(proposition, ProofRule::IntegerFloatExact));
        }
        let integer_conversion = |proposition: &Proposition| {
            matches!(
                proposition,
                Proposition::FloatIntegerConversion(_) | Proposition::IeeeFloatIntegerExact { .. }
            )
        };
        let judgment = if integer_conversion(&proposition)
            || matches!(&proposition, Proposition::Implication { premise, conclusion } if **conclusion == Proposition::Falsehood && integer_conversion(premise))
        {
            PrimitiveJudgment::ClosedFloatIntegerRelation
        } else if matches!(proposition, Proposition::ScalarIeeeFloatComparison { .. })
            || matches!(&proposition, Proposition::Implication { premise, conclusion }
                if **conclusion == Proposition::Falsehood && matches!(**premise, Proposition::ScalarIeeeFloatComparison { .. }))
        {
            PrimitiveJudgment::ClosedIeeeFloatRelation
        } else {
            PrimitiveJudgment::ClosedIntegerRelation
        };
        decide_primitive(self.context, &proposition, judgment)
            .ok()
            .map(|()| primitive(proposition, judgment))
    }

    fn reflexive(&self, proposition: Proposition) -> Option<ProofNode> {
        decide_primitive(
            self.context,
            &proposition,
            PrimitiveJudgment::ReflexiveEquality,
        )
        .ok()
        .map(|()| primitive(proposition, PrimitiveJudgment::ReflexiveEquality))
    }

    /// The tightest literal bounds this prover can prove on an integer
    /// `value`, lower then upper, leaving out a bound that is only its
    /// carrier's. Each is found with the proof that a goal stating it would
    /// get, so a caller may state either as a goal here and have it proved.
    pub(crate) fn literal_bounds(
        &mut self,
        value: ValueId,
    ) -> (Option<IntegerValue>, Option<IntegerValue>) {
        let Some(ScalarType::Integer(integer)) = self.values.scalar_type(value) else {
            return (None, None);
        };
        // A rigid width has no concrete endpoint. Reporting bounds here
        // must not turn an unknown carrier into a literal interval.
        let (Some(minimum), Some(maximum)) = (integer.minimum_value(), integer.maximum_value())
        else {
            return (None, None);
        };
        let narrower = |literal: Option<IntegerValue>, carrier: IntegerValue| {
            literal.filter(|literal| integer.compare(*literal, carrier) != Some(Ordering::Equal))
        };
        let mut lower = narrower(self.bound_literal(value, Direction::Lower), minimum);
        let mut upper = narrower(self.bound_literal(value, Direction::Upper), maximum);
        // A bound that holds only by cases is no bound of the value's own:
        // under `c == true` with `c` the join of `20 <= v && v <= 60`, the
        // limits are inside a disjunctive fact. Each literal such a fact
        // compares the value with, and its neighbours, is tried as a goal,
        // which `prove` splits; the narrowest that holds is taken. A side
        // the value already has a bound on is left as it is: the search is
        // for what nothing else says, and each try costs a proof search.
        let subject = ScalarTerm::value(value, ScalarType::Integer(integer));
        let mut compared = Vec::new();
        if lower.is_none() || upper.is_none() {
            for fact in &self.facts {
                if matches!(fact.proposition, Proposition::Disjunction(_)) {
                    compared_literals(&fact.proposition, &subject, &mut compared);
                }
            }
        }
        let (try_lower, try_upper) = (lower.is_none(), upper.is_none());
        let mut candidates = Vec::new();
        for literal in compared {
            for candidate in [step(literal, false), Some(literal), step(literal, true)]
                .into_iter()
                .flatten()
            {
                if !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
        }
        candidates.truncate(INTERVAL_CANDIDATES);
        let narrows = |candidate: IntegerValue, known: Option<IntegerValue>, wanted: Ordering| {
            known.is_none_or(|known| integer.compare(candidate, known) == Some(wanted))
        };
        for candidate in candidates {
            let Ok(literal) = ScalarTerm::integer(integer, candidate) else {
                continue;
            };
            if try_lower
                && narrows(candidate, lower, Ordering::Greater)
                && integer.compare(candidate, minimum) != Some(Ordering::Equal)
                && self
                    .prove(&Proposition::LessOrEqual(literal.clone(), subject.clone()))
                    .is_ok()
            {
                lower = Some(candidate);
            }
            if try_upper
                && narrows(candidate, upper, Ordering::Less)
                && integer.compare(candidate, maximum) != Some(Ordering::Equal)
                && self
                    .prove(&Proposition::LessOrEqual(subject.clone(), literal))
                    .is_ok()
            {
                upper = Some(candidate);
            }
        }
        (lower, upper)
    }

    /// A certificate whose conclusion is exactly `goal`: directly, or by a
    /// case split over a disjunctive fact.
    pub(crate) fn prove(&mut self, goal: &Proposition) -> Result<ProofNode, Unproven> {
        if self.active_goals.contains(goal) {
            return Err("recursive goal under unchanged premises");
        }
        self.active_goals.push(goal.clone());
        let result = self.prove_goal(goal);
        self.active_goals.pop();
        result
    }

    fn prove_direct_subgoal(&mut self, goal: &Proposition) -> Result<ProofNode, Unproven> {
        let case_split = std::mem::replace(&mut self.case_split, false);
        let result = self.prove(goal);
        self.case_split = case_split;
        result
    }

    fn prove_goal(&mut self, goal: &Proposition) -> Result<ProofNode, Unproven> {
        let direct = self.prove_direct(goal);
        if direct.is_ok()
            || !self.case_split
            || !matches!(
                goal,
                Proposition::LessOrEqual(..)
                    | Proposition::LessThan(..)
                    | Proposition::Equal(..)
                    | Proposition::IntegerMathLessOrEqual(..)
                    // `n == 1 or n == 2` proves `n == 1 or n == 2 or
                    // n == 3`, although no one alternative holds alone.
                    | Proposition::Disjunction(_)
                    // Owed where nothing may happen: a crash site whose
                    // machine publishes no route for its cause. It holds
                    // only where what is known is contradictory.
                    | Proposition::Falsehood
            ) && !covered(goal, &|_| false, &|_| false)
        {
            return direct;
        }
        if self.assumptions.local.is_empty()
            && self
                .facts
                .iter()
                .any(|fact| matches!(fact.proposition, Proposition::Disjunction(_)))
            && self.has_scalar_counterexample(goal)
        {
            return direct;
        }
        if let Some(split) = self.split(goal) {
            return Ok(split);
        }
        self.unreachable(goal)
            .ok_or(if self.remaining_case_branches == 0 {
                "the bounded disjunction search was exhausted"
            } else {
                direct.err().unwrap_or("no proof")
            })
    }

    /// `goal` at a point no execution reaches: what is known there is
    /// contradictory, so anything follows (`wiki/spec/proofs/contracts.md`,
    /// "Required automatic discharge": order reasoning includes
    /// "contradiction of an unreachable path"). Each search ends in
    /// `ex_falso`: a closed mathematical comparison proved false, an order
    /// chained with its opposite (`refute`, which needs a literal endpoint),
    /// or a taken guard whose negation is provable (`refute_guard`). This
    /// search is tried last, so a reachable point pays for it only when its
    /// goal is otherwise unproved.
    fn unreachable(&mut self, goal: &Proposition) -> Option<ProofNode> {
        if std::mem::replace(&mut self.refuting, true) {
            return None;
        }
        #[cfg(test)]
        {
            self.activity.contradiction_searches += 1;
        }
        let absurd = self.contradiction(goal);
        self.refuting = false;
        absurd
    }

    fn contradiction(&mut self, goal: &Proposition) -> Option<ProofNode> {
        if let Some(absurd) = self
            .refute_closed_relation(goal)
            .or_else(|| self.refute_ieee_relations(goal))
        {
            return Some(absurd);
        }
        // Facts stay fixed during this search. Build a candidate's proof
        // only after its opposite succeeds; a failed search cannot use it.
        // Keep orders, literal equalities and guards in the same order.
        for index in 0..self.facts.len() {
            let fact = &self.facts[index];
            if fact.axiom.is_some() || order_with_closed_endpoint(&fact.proposition).is_none() {
                continue;
            }
            let proposition = fact.proposition.clone();
            if let Some(refuted) = self.refute(
                &proposition,
                |facts, activity| Some(fact_proof_for_refutation(facts, index, activity)),
                goal,
            ) {
                return Some(refuted);
            }
        }
        // An equality with a literal supplies both directions. Either can
        // contradict another bound, so try each before the next equality.
        for index in 0..self.facts.len() {
            let fact = &self.facts[index];
            if fact.axiom.is_some()
                || !matches!(&fact.proposition, Proposition::Equal(left, right)
                    if left.integer_value().is_some() != right.integer_value().is_some())
            {
                continue;
            }
            let Proposition::Equal(left, right) = fact.proposition.clone() else {
                unreachable!("only a literal equality reaches this candidate");
            };
            let forward = Proposition::LessOrEqual(left.clone(), right.clone());
            if let Some(refuted) = self.refute(
                &forward,
                |facts, activity| weaken(fact_proof_for_refutation(facts, index, activity)).ok(),
                goal,
            ) {
                return Some(refuted);
            }
            let reverse = Proposition::LessOrEqual(right, left);
            if let Some(refuted) = self.refute(
                &reverse,
                |facts, activity| {
                    flip(fact_proof_for_refutation(facts, index, activity))
                        .and_then(weaken)
                        .ok()
                },
                goal,
            ) {
                return Some(refuted);
            }
        }
        for index in 0..self.facts.len() {
            let fact = &self.facts[index];
            if fact.axiom.is_some() || !is_branch_fact(&fact.proposition) {
                continue;
            }
            let taken = fact.proposition.clone();
            if let Some(refuted) = self.refute_guard(
                &taken,
                |facts, activity| Some(fact_proof_for_refutation(facts, index, activity)),
                goal,
            ) {
                return Some(refuted);
            }
        }
        None
    }

    /// A closed false scalar or mathematical assumption makes a path unreachable.
    /// Retain its exact value equations in the proof; the kernel independently
    /// evaluates the negated comparison and checks the transport back.
    fn refute_closed_relation(&mut self, goal: &Proposition) -> Option<ProofNode> {
        for index in 0..self.facts.len() {
            if self.facts[index].axiom.is_some()
                || !matches!(
                    &self.facts[index].proposition,
                    Proposition::IntegerMathEqual(..)
                        | Proposition::IntegerMathLessThan(..)
                        | Proposition::IntegerMathLessOrEqual(..)
                        | Proposition::ScalarIeeeFloatComparison { .. }
                )
            {
                continue;
            }
            let negation = Proposition::Implication {
                premise: Box::new(self.facts[index].proposition.clone()),
                conclusion: Box::new(Proposition::Falsehood),
            };
            self.select_transport_inputs(&negation);
            let inputs = &self.transport.selected;
            let Ok(closed) = check_value_equality_denotation(
                self.context,
                &negation,
                inputs
                    .iter()
                    .map(|&index| &self.facts[self.equations[index]].proposition),
            ) else {
                continue;
            };
            let Some(proof) = self.closed(closed) else {
                continue;
            };
            let negation = node(
                negation,
                ProofRule::ValueEqualityTransport {
                    premise: Box::new(proof),
                    equalities: inputs
                        .iter()
                        .map(|&index| self.facts[self.equations[index]].proof(&self.facts))
                        .collect(),
                },
            );
            let absurd = node(
                Proposition::Falsehood,
                ProofRule::ImplicationElimination {
                    implication: Box::new(negation),
                    premise: Box::new(self.facts[index].proof(&self.facts)),
                },
            );
            return Some(ex_falso(absurd, goal));
        }
        None
    }

    /// `goal` where the guard that was taken cannot have held: `taken` is the
    /// branch fact `c == b`, and `c == not b` is provable from the rest.
    ///
    /// This is how two order facts over values with no literal between them
    /// contradict. After `requires low <= high`, the arm of `high < low`
    /// knows `c == true` with `c` defined as `high < low`. Chaining the two
    /// orders only reaches `high < high`, which the kernel has no rule to
    /// call false: its closed relations need literals. But `c == false`
    /// denotes `low <= high` under `c`'s definition, which is known, so
    /// `true == c` and `c == false` give `true == false`, and that the
    /// kernel's denotation already calls `Falsehood`. Every step is an
    /// existing rule (`ValueEqualityTransport`, `EqualitySymmetry`,
    /// `EqualityTransitivity`, `PredicateDenotation`); no order arithmetic is
    /// done, so nothing here depends on signedness, width or overflow.
    ///
    /// The negated guard's denotation is proved without a split or a nested
    /// contradiction search, so this cannot re-enter itself.
    fn refute_guard(
        &mut self,
        taken: &Proposition,
        evidence: impl FnOnce(&[Fact], &mut ProofSearchActivity) -> Option<ProofNode>,
        goal: &Proposition,
    ) -> Option<ProofNode> {
        let Proposition::Equal(condition, ScalarTerm::Boolean(value)) = taken else {
            return None;
        };
        let untaken = Proposition::Equal(condition.clone(), ScalarTerm::Boolean(!*value));
        let absurd = Proposition::Equal(ScalarTerm::Boolean(*value), ScalarTerm::Boolean(!*value));
        let denotation = check_value_equality_denotation(
            self.context,
            &untaken,
            self.equations
                .iter()
                .map(|&index| &self.facts[index].proposition),
        )
        .ok();
        let transported =
            if let Some(denotation) = denotation.filter(|denotation| *denotation != untaken) {
                let nested = std::mem::replace(&mut self.case_split, false);
                let premise = self.prove_direct(&denotation);
                self.case_split = nested;
                premise.ok().map(|premise| {
                    node(
                        untaken.clone(),
                        ProofRule::ValueEqualityTransport {
                            premise: Box::new(premise),
                            equalities: self
                                .equations
                                .iter()
                                .map(|&index| self.facts[index].proof(&self.facts))
                                .collect(),
                        },
                    )
                })
            } else {
                None
            };
        let untaken = match transported {
            Some(untaken) => untaken,
            None => {
                // When `condition` has no definition (as at a Boolean join) or
                // its unfolded form does not close alone, check direct
                // hypotheses on `condition` itself.
                let nested = std::mem::replace(&mut self.case_split, false);
                let direct = self.prove_direct(&untaken);
                self.case_split = nested;
                direct.ok()?
            }
        };
        let taken = evidence(&self.facts, self.activity)?;
        let absurd = node(
            absurd,
            ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(flip(taken).ok()?),
                middle_equals_right: Box::new(untaken),
            },
        );
        Some(ex_falso(absurd, goal))
    }

    /// `goal` from a disjunctive fact (a disequality guard transports to
    /// `v < k or k < v`): each alternative either proves the goal as a local
    /// assumption or contradicts what is already known.
    fn split(&mut self, goal: &Proposition) -> Option<ProofNode> {
        for index in 0..self.facts.len() {
            // No branch can start once the shared budget is exhausted. An
            // active cut likewise supplies no new information. Check both
            // before expanding its owned proof tree or cloning alternatives.
            if self.remaining_case_branches == 0 {
                return None;
            }
            let fact = &self.facts[index];
            let Proposition::Disjunction(parts) = &fact.proposition else {
                continue;
            };
            let origin = fact.disjunction_origin(&self.facts);
            if self.active_cuts.iter().any(|active| {
                active.proposition == fact.proposition
                    || origin
                        .as_ref()
                        .is_some_and(|origin| active.origin.as_ref() == Some(origin))
            }) {
                continue;
            }
            let parts = parts.clone();
            self.active_cuts.push(ActiveCut {
                proposition: fact.proposition.clone(),
                origin,
            });
            let mut branches = Vec::with_capacity(parts.len());
            for part in &parts {
                let Some(branch) = self.branch(part, goal) else {
                    break;
                };
                branches.push(branch);
            }
            self.active_cuts.pop();
            if branches.len() != parts.len() {
                continue;
            }
            // Failed cuts need no owned derivation of the disjunction. Facts
            // stay fixed while branches are checked, so expand this exact
            // selected source only after every alternative has proved the goal.
            let proof = self.facts[index].proof(&self.facts);
            #[cfg(test)]
            {
                self.activity.disjunction_materializations += 1;
            }
            return Some(node(
                goal.clone(),
                ProofRule::DisjunctionElimination {
                    disjunction: Box::new(proof),
                    branches,
                },
            ));
        }
        None
    }

    /// One alternative: the kernel adds it as the next assumption.
    fn branch(&mut self, part: &Proposition, goal: &Proposition) -> Option<ProofNode> {
        self.remaining_case_branches = self.remaining_case_branches.checked_sub(1)?;
        if order_with_closed_endpoint(part).is_some() {
            let local = node(
                part.clone(),
                ProofRule::Assumption {
                    index: self.assumptions.len(),
                },
            );
            if let Some(contradiction) = self.refute(part, |_, _| Some(local), goal) {
                return Some(contradiction);
            }
        }
        self.prove_under(part, goal).ok()
    }

    fn prove_under(
        &mut self,
        premise: &Proposition,
        goal: &Proposition,
    ) -> Result<ProofNode, Unproven> {
        let mut assumptions = self.assumptions.clone();
        assumptions.local.push(premise);
        let mut branch = Prover::prepare(
            self.context,
            assumptions,
            self.axioms,
            self.values,
            self.activity,
            Some((&self.facts, &self.normalized_premises)),
        );
        branch.case_split = self.case_split;
        branch.remaining_case_branches = self.remaining_case_branches;
        // The parent resumes after this branch. Nested cuts are popped by
        // split on both a successful proof and a failed alternative.
        branch.active_cuts = std::mem::take(&mut self.active_cuts);
        let proof = branch.prove(goal);
        self.active_cuts = branch.active_cuts;
        self.remaining_case_branches = branch.remaining_case_branches;
        proof
    }

    /// `goal` from an alternative that contradicts what is known: chained
    /// with the opposite order it gives a closed false relation, which
    /// `ex_falso` turns into `goal`.
    fn refute(
        &mut self,
        relation: &Proposition,
        evidence: impl FnOnce(&[Fact], &mut ProofSearchActivity) -> Option<ProofNode>,
        goal: &Proposition,
    ) -> Option<ProofNode> {
        let (strict, left, right) = order_with_closed_endpoint(relation)?;
        let (left, right) = (left.clone(), right.clone());
        let opposite = if strict {
            Proposition::LessOrEqual(right.clone(), left.clone())
        } else {
            Proposition::LessThan(right.clone(), left.clone())
        };
        let opposite = self.prove_direct(&opposite).ok()?;
        // A failed opposite search cannot use the local proof tree. Build
        // it only here, from the same facts and with the same conclusion.
        let local = evidence(&self.facts, self.activity)?;
        let false_relation = if left.integer_value().is_some() {
            chain(local, opposite).ok()?
        } else {
            chain(opposite, local).ok()?
        };
        Some(ex_falso(self.at_literals(false_relation)?, goal))
    }

    /// `relation` restated so that both its endpoints are plain literals.
    ///
    /// `refute` chains through any endpoint that has a value, and a closed
    /// term such as `(21 <<% 1) >>% 1` has one, but the kernel's denotation
    /// calls a relation `Falsehood` only between plain literals
    /// (`closed_integer_relation`). Chained as it stood, `T < T` reached
    /// `ex_falso` undecided and the kernel refused the certificate
    /// (`RuleConclusionMismatch("predicate denotation")`), which blocked
    /// programs whose untaken arm calls a crashing state. Each closed endpoint
    /// gains one closed step to its value, as `literal_endpoint` does, so the
    /// relation becomes `21 < 21`, which the kernel does decide.
    fn at_literals(&self, relation: ProofNode) -> Option<ProofNode> {
        let (_, left, right) = order(&relation.conclusion)?;
        let (left, right) = (left.clone(), right.clone());
        let plain = |term: &ScalarTerm| {
            let (integer, value) = term.integer_value()?;
            ScalarTerm::integer(integer, value).ok()
        };
        let mut relation = relation;
        if !matches!(left, ScalarTerm::Integer { .. }) {
            let value = plain(&left)?;
            relation = chain(
                self.closed(Proposition::LessOrEqual(value, left))?,
                relation,
            )
            .ok()?;
        }
        if !matches!(right, ScalarTerm::Integer { .. }) {
            let value = plain(&right)?;
            relation = chain(
                relation,
                self.closed(Proposition::LessOrEqual(right, value))?,
            )
            .ok()?;
        }
        Some(relation)
    }

    /// `premise => conclusion`: the kernel adds the premise as the next
    /// assumption and checks the conclusion under it, as it does a split
    /// alternative.
    fn implication(
        &mut self,
        premise: &Proposition,
        conclusion: &Proposition,
        goal: &Proposition,
    ) -> Result<ProofNode, Unproven> {
        let body = self.prove_under(premise, conclusion)?;
        Ok(node(
            goal.clone(),
            ProofRule::ImplicationIntroduction {
                body: Box::new(body),
            },
        ))
    }

    fn prove_direct(&mut self, goal: &Proposition) -> Result<ProofNode, Unproven> {
        // Proving Falsehood already asks whether these premises contradict.
        // An implication's antecedent cannot usefully restart that same search;
        // the outer goal still tries its ordinary cases and contradiction pass.
        let refuting = self.refuting;
        self.refuting |= matches!(goal, Proposition::Falsehood);
        let result = match self.prove_structural(goal) {
            Ok(proof) => Ok(proof),
            Err(detail) => self
                .by_implication(goal)
                .or_else(|| self.by_transport(goal))
                .ok_or(detail),
        };
        self.refuting = refuting;
        result
    }

    fn prove_structural(&mut self, goal: &Proposition) -> Result<ProofNode, Unproven> {
        if let Some(index) = self.exact_fact(goal) {
            return Ok(self.facts[index].proof(&self.facts));
        }
        if let Some(index) = self.exact_fact(&Proposition::Falsehood) {
            return Ok(ex_falso(self.facts[index].proof(&self.facts), goal));
        }
        if let Some(proof) = self.order_through_cast(goal) {
            return Ok(proof);
        }
        match goal {
            Proposition::Truth => Ok(primitive(Proposition::Truth, PrimitiveJudgment::Truth)),
            Proposition::DeclarationIdentity(_) => Err("a typed Identity needs mathematical evidence"),
            Proposition::Falsehood => Err("the goal is false"),
            Proposition::Conjunction(parts) => {
                let proofs = parts
                    .iter()
                    .map(|part| self.prove(part))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(node(
                    goal.clone(),
                    ProofRule::ConjunctionIntroduction(proofs),
                ))
            }
            Proposition::Disjunction(parts) => {
                // Try direct introduction for every alternative before
                // splitting the premises for the whole disjunction.
                let case_split = std::mem::replace(&mut self.case_split, false);
                let mut last = "no disjunct is provable";
                for (index, part) in parts.iter().enumerate() {
                    match self.prove(part) {
                        Ok(disjunct) => {
                            self.case_split = case_split;
                            return Ok(node(
                                goal.clone(),
                                ProofRule::DisjunctionIntroduction {
                                    disjunct: Box::new(disjunct),
                                    index,
                                },
                            ));
                        }
                        Err(reason) => last = reason,
                    }
                }
                self.case_split = case_split;
                Err(last)
            }
            Proposition::LessOrEqual(left, right) | Proposition::LessThan(left, right) => {
                if let Some(proof) = self.closed(goal.clone()) {
                    return Ok(proof);
                }
                // Carrier endpoints need no search through unrelated facts.
                // The kernel checks the live value's exact type and endpoint.
                if decide_primitive(self.context, goal, PrimitiveJudgment::IntegerCarrierBound)
                    .is_ok()
                {
                    return Ok(primitive(
                        goal.clone(),
                        PrimitiveJudgment::IntegerCarrierBound,
                    ));
                }
                let strict = matches!(goal, Proposition::LessThan(..));
                self.compare(left, right, strict)
            }
            Proposition::Equal(left, right) => {
                if left == right {
                    return Ok(primitive(
                        goal.clone(),
                        PrimitiveJudgment::ReflexiveEquality,
                    ));
                }
                let reversed = Proposition::Equal(right.clone(), left.clone());
                if let Some(fact) = self.facts.iter().find(|fact| fact.proposition == reversed)
                    && let Ok(flipped) = flip(fact.proof(&self.facts))
                {
                    return Ok(flipped);
                }
                if let Some(proof) = self.closed(goal.clone()) {
                    return Ok(proof);
                }
                if !matches!(left.scalar_type(), ScalarType::Integer(_)) {
                    return Err("equality of non-integers is proved only by a hypothesis");
                }
                let forward = self.prove(&Proposition::LessOrEqual(left.clone(), right.clone()))?;
                let backward =
                    self.prove(&Proposition::LessOrEqual(right.clone(), left.clone()))?;
                Ok(node(
                    goal.clone(),
                    ProofRule::IntegerOrderAntisymmetry {
                        forward: Box::new(forward),
                        backward: Box::new(backward),
                    },
                ))
            }
            Proposition::IntegerMathLessOrEqual(left, right) => {
                if let Some(proof) = self.closed(goal.clone()) {
                    return Ok(proof);
                }
                let binders = self.width_binders();
                let width = |value| binders.contains(&value);
                if widths::mentions_parameter_width(left, &width)
                    || widths::mentions_parameter_width(right, &width)
                {
                    return self.width_goal(left, right);
                }
                if let Some(proof) = self.carrier_order(goal) {
                    return Ok(proof);
                }
                if let Some(proof) = self.mathematical_value_order(goal, left, right) {
                    return Ok(proof);
                }
                self.arithmetic(left, right)
            }
            Proposition::IntegerMathEqual(..) => self
                .reflexive(goal.clone())
                .or_else(|| self.closed(goal.clone()))
                .ok_or("non-reflexive mathematical equality over open terms"),
            Proposition::IntegerMathLessThan(left, right) => self
                .closed(goal.clone())
                .or_else(|| self.strict_sum_bound(left, right))
                .ok_or("mathematical strict order over open terms"),
            Proposition::Atom(_)
            | Proposition::IeeeFloatComparison { .. }
            | Proposition::ByteSequenceEqual { .. }
            | Proposition::StructuralIdentity { .. }
            | Proposition::StructuralCaseMembership { .. }
            | Proposition::ContentConservation(_) => Err("goal form outside this prover"),
            Proposition::FloatMeaningEqual(_) => self.float_meaning(goal),
            Proposition::IntegerIeeeFloatExact { .. } => {
                proof_admission::check_integer_float_exact(self.context, goal)
                    .map(|()| node(goal.clone(), ProofRule::IntegerFloatExact))
                    .map_err(|error| match error {
                        proof_admission::FloatIntegerError::NotEstablished => {
                            "the live integer facts do not establish an exact float cast"
                        }
                        proof_admission::FloatIntegerError::Malformed(_) => {
                            "exact float cast proof has a malformed source"
                        }
                        proof_admission::FloatIntegerError::Unsupported(detail) => detail,
                        proof_admission::FloatIntegerError::Semantic(_) => {
                            "exact float cast semantic evaluation failed"
                        }
                        proof_admission::FloatIntegerError::Transport(_) => {
                            "exact float cast source substitution failed"
                        }
                    })
            }
            Proposition::IeeeFloatIntegerExact { .. } => self
                .closed(goal.clone())
                .or_else(|| self.float_integer_conversion(goal))
                .ok_or("the live float facts do not establish an exact integer cast"),
            Proposition::FloatIntegerConversion(_) => self
                .closed(goal.clone())
                .ok_or("the integer conversion result relation is not established"),
            Proposition::ScalarIeeeFloatComparison { .. } => self
                .closed(goal.clone())
                .or_else(|| self.ieee_float_order(goal))
                .or_else(|| self.float_integer_conversion(goal))
                .ok_or("the IEEE premises do not establish this comparison"),
            Proposition::Implication {
                premise,
                conclusion,
            } => self.implication(premise, conclusion, goal),
        }
    }

    fn by_implication(&mut self, goal: &Proposition) -> Option<ProofNode> {
        for position in self.implications.candidates(goal) {
            let index = self.implications.positions[position].1;
            let Proposition::Implication {
                premise,
                conclusion,
            } = &self.facts[index].proposition
            else {
                continue;
            };
            if !conclusion.same_commutative_predicate(goal) {
                continue;
            }
            let premise = premise.clone();
            let conclusion = conclusion.as_ref().clone();
            let Ok(premise) = self.prove(&premise) else {
                continue;
            };
            let result = node(
                conclusion,
                ProofRule::ImplicationElimination {
                    implication: Box::new(self.facts[index].proof(&self.facts)),
                    premise: Box::new(premise),
                },
            );
            if result.conclusion == *goal {
                return Some(result);
            }
            if let Ok(flipped) = flip(result) {
                return Some(flipped);
            }
        }
        None
    }

    /// Lift a same-carrier order without changing either operand.
    fn carrier_order(&mut self, goal: &Proposition) -> Option<ProofNode> {
        let relation = lower_integer_math_relation(goal)?;
        let Proposition::LessOrEqual(left, _) = &relation else {
            return None;
        };
        // Compound carrier terms take the arithmetic route. A bare value
        // relation can use the existing branch and contract premises.
        let unchanged = self.reflexive(Proposition::Equal(left.clone(), left.clone()))?;
        let proof = self.prove_direct_subgoal(&relation).ok()?;
        Some(node(
            goal.clone(),
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(proof),
                equality: Box::new(unchanged),
                endpoint: 0,
            },
        ))
    }

    /// For an exact sum, a strict bound on either operand leaves room for
    /// one more. Translate that bound by the other operand, then use the
    /// ordinary arithmetic prover to bound the larger sum.
    fn strict_sum_bound(
        &mut self,
        sum: &IntegerMathTerm,
        limit: &IntegerMathTerm,
    ) -> Option<ProofNode> {
        let IntegerMathTerm::Add(left, right) = sum else {
            return None;
        };
        for (operand, addend) in [(left, right), (right, left)] {
            let IntegerMathTerm::MathValue { source_type, value } = operand.as_ref() else {
                continue;
            };
            let scalar = ScalarTerm::value(*value, ScalarType::Integer(*source_type));
            for index in 0..self.facts.len() {
                let fact = &self.facts[index];
                let Proposition::LessThan(low, high) = &fact.proposition else {
                    continue;
                };
                if low != &scalar || !is_bare_scalar(high) {
                    continue;
                }
                let Some(high) = IntegerMathTerm::embed(high.clone()) else {
                    continue;
                };
                let upper_sum = IntegerMathTerm::Add(Box::new(high.clone()), addend.clone());
                let strict = fact.proof(&self.facts);
                let Ok(upper) = self.arithmetic(&upper_sum, limit) else {
                    continue;
                };
                let successor = IntegerMathTerm::Add(
                    operand.clone(),
                    Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
                );
                let middle = IntegerMathTerm::Add(Box::new(successor.clone()), addend.clone());
                let Some(first) =
                    self.closed(Proposition::IntegerMathLessThan(sum.clone(), middle))
                else {
                    continue;
                };
                let discrete = node(
                    Proposition::IntegerMathLessOrEqual(successor, high),
                    ProofRule::IntegerOrderDiscreteness {
                        relation: Box::new(strict),
                    },
                );
                let translated = translate(discrete, addend.as_ref().clone())?;
                let remaining = chain_math(translated, upper)?;
                return Some(node(
                    Proposition::IntegerMathLessThan(sum.clone(), limit.clone()),
                    ProofRule::IntegerStrictOrderTransitivity {
                        left_to_middle: Box::new(first),
                        middle_to_right: Box::new(remaining),
                    },
                ));
            }
        }
        None
    }

    /// Unfold only the sum when proving its order. Other operand definitions
    /// stay in scope for transport, including a conversion that the sum
    /// names as one checked value.
    fn sum_order(&mut self, goal: &Proposition) -> Option<ProofNode> {
        let (strict, left, right) = order(goal)?;
        let mut equalities = Vec::new();
        let mut sum_operand = |operand: &ScalarTerm| {
            let ScalarTerm::Value { id, .. } = operand else {
                return operand.clone();
            };
            let Some(Some(definition)) = self.definitions.get(*id) else {
                return operand.clone();
            };
            let fact = &self.facts[self.equations[*definition]];
            let Proposition::Equal(value, sum @ ScalarTerm::ExactIntegerAdd { .. }) =
                &fact.proposition
            else {
                return operand.clone();
            };
            if value != operand {
                return operand.clone();
            }
            equalities.push(fact.proof(&self.facts));
            sum.clone()
        };
        let left = sum_operand(left);
        let right = sum_operand(right);
        let unfolded = if strict {
            Proposition::LessThan(left, right)
        } else {
            Proposition::LessOrEqual(left, right)
        };
        let lifted = lift_exact_integer_order(&unfolded)?;
        let (left, right) = match &lifted {
            Proposition::IntegerMathLessOrEqual(left, right)
            | Proposition::IntegerMathLessThan(left, right) => (left, right),
            _ => return None,
        };
        let endpoint = match (left, right) {
            (IntegerMathTerm::Add(..), _) => 0,
            (_, IntegerMathTerm::Add(..)) => 1,
            _ => return None,
        };
        let term = if endpoint == 0 { left } else { right };
        let unchanged = primitive(
            Proposition::IntegerMathEqual(term.clone(), term.clone()),
            PrimitiveJudgment::ReflexiveEquality,
        );
        let relation = self
            .prove_direct_subgoal(&lifted)
            .ok()
            .or_else(|| self.strict_sum_order(&unfolded, &lifted))?;
        let proof = node(
            unfolded,
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(relation),
                equality: Box::new(unchanged),
                endpoint,
            },
        );
        if equalities.is_empty() {
            return Some(proof);
        }
        Some(node(
            goal.clone(),
            ProofRule::ValueEqualityTransport {
                premise: Box::new(proof),
                equalities,
            },
        ))
    }

    /// Preserve a strict remaining-capacity relation that separate literal
    /// operand ranges lose. Search only existing order edges and equations;
    /// the requested sum is never a premise of that search.
    fn strict_sum_order(
        &mut self,
        finite: &Proposition,
        mathematical: &Proposition,
    ) -> Option<ProofNode> {
        let Proposition::LessThan(
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            },
            limit,
        ) = finite
        else {
            return None;
        };
        if scalar_type.carrier() != IntegerCarrier::Fixed || scalar_type.is_address() {
            return None;
        }
        let Proposition::IntegerMathLessThan(sum, upper) = mathematical else {
            return None;
        };
        for (subject, offset) in [(left, right), (right, left)] {
            if !matches!(subject.as_ref(), ScalarTerm::Value { .. }) {
                continue;
            }
            let Ok(remaining) =
                ScalarTerm::exact_integer_subtract(*scalar_type, limit.clone(), (**offset).clone())
            else {
                continue;
            };
            // The strict path must cite existing order facts or equations for
            // this exact difference. A <= path is not enough.
            let Some(strict) = self.relation(subject, &remaining, true) else {
                continue;
            };
            let one = IntegerMathTerm::literal(IntegerValue::Unsigned(1));
            let discrete = node(
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::Add(
                        Box::new(IntegerMathTerm::embed((**subject).clone())?),
                        Box::new(one.clone()),
                    ),
                    IntegerMathTerm::embed(remaining)?,
                ),
                ProofRule::IntegerOrderDiscreteness {
                    relation: Box::new(strict),
                },
            );
            let successor = IntegerMathTerm::Add(Box::new(sum.clone()), Box::new(one));
            let Some(at_most) = self.translate_mathematical_order(
                discrete,
                IntegerMathTerm::embed((**offset).clone())?,
                &Proposition::IntegerMathLessOrEqual(successor.clone(), upper.clone()),
            ) else {
                continue;
            };
            let below_successor =
                self.closed(Proposition::IntegerMathLessThan(sum.clone(), successor))?;
            return Some(node(
                mathematical.clone(),
                ProofRule::IntegerStrictOrderTransitivity {
                    left_to_middle: Box::new(below_successor),
                    middle_to_right: Box::new(at_most),
                },
            ));
        }
        None
    }

    /// An admitted exact conversion preserves the mathematical value in
    /// either endpoint of an inequality. Prove the original order with the
    /// other operand unchanged, then cite the defining conversion in the
    /// kernel's checked cast chain. Lossy conversions have no such witness.
    fn order_through_cast(&mut self, goal: &Proposition) -> Option<ProofNode> {
        let math;
        let (left, right, strict) = match goal {
            Proposition::IntegerMathLessOrEqual(left, right) => (left, right, false),
            Proposition::IntegerMathLessThan(left, right) => (left, right, true),
            Proposition::LessOrEqual(left, right) | Proposition::LessThan(left, right) => {
                if ![left, right].iter().any(|term| matches!(term, ScalarTerm::Value { id, .. } if self.cast_definition(*id).is_some())) {
                    return None;
                }
                math = proof_admission::lift_fixed_integer_relation(goal)?;
                match &math {
                    Proposition::IntegerMathLessOrEqual(left, right) => (left, right, false),
                    Proposition::IntegerMathLessThan(left, right) => (left, right, true),
                    _ => return None,
                }
            }
            _ => return None,
        };
        for (endpoint, at_left) in [(left, true), (right, false)] {
            let IntegerMathTerm::MathValue { value, source_type } = endpoint else {
                continue;
            };
            let Some((index, operand)) = self.cast_definition(*value) else {
                continue;
            };
            let root = operand.clone();
            let witness = proof_admission::IntegerCastChainWitness {
                root: root.clone(),
                target: ScalarTerm::value(*value, ScalarType::Integer(*source_type)),
                definition_axioms: vec![index],
            };
            let Ok(chain) = proof_admission::check_integer_cast_chain_witness(
                self.context,
                self.axioms,
                &witness,
            ) else {
                continue;
            };
            let Some(origin) = IntegerMathTerm::embed(root) else {
                continue;
            };
            let (low, high) = if at_left {
                (origin, right.clone())
            } else {
                (left.clone(), origin)
            };
            let source = if strict {
                Proposition::IntegerMathLessThan(low, high)
            } else {
                Proposition::IntegerMathLessOrEqual(low, high)
            };
            let upstream = lower_integer_math_relation(&source)
                .and_then(|finite| self.prove_direct_subgoal(&finite).ok())
                .or_else(|| self.prove_direct_subgoal(&source).ok());
            if let Some(premise) = upstream
                && proof_admission::check_integer_cast_bound_conversion(
                    &chain,
                    &premise.conclusion,
                    goal,
                )
                .is_ok()
            {
                return Some(node(
                    goal.clone(),
                    ProofRule::IntegerCastBound {
                        root_bound: Box::new(premise),
                        witness,
                    },
                ));
            }
        }
        None
    }

    fn cast_definition(&self, value: ValueId) -> Option<(usize, &ScalarTerm)> {
        let equation = self.equations[(*self.definitions.get(value)?)?];
        let fact = &self.facts[equation];
        let Proposition::Equal(
            ScalarTerm::Value { id, .. },
            ScalarTerm::IntegerExactCast { operand, .. } | ScalarTerm::IntegerWiden { operand, .. },
        ) = &fact.proposition
        else {
            return None;
        };
        (*id == value).then_some((fact.axiom?, operand.as_ref()))
    }

    /// Compare mathematical values using ordered upper and lower bounds.
    fn mathematical_value_order(
        &mut self,
        goal: &Proposition,
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Option<ProofNode> {
        if !matches!(
            (left, right),
            (
                IntegerMathTerm::MathValue { .. },
                IntegerMathTerm::MathValue { .. }
            )
        ) {
            return None;
        }
        let upper = self.arithmetic_image(left, Direction::Upper).ok()?;
        let lower = self.arithmetic_image(right, Direction::Lower).ok()?;
        let (
            Proposition::IntegerMathLessOrEqual(_, left_limit),
            Proposition::IntegerMathLessOrEqual(right_limit, _),
        ) = (&upper.conclusion, &lower.conclusion)
        else {
            return None;
        };
        let bridge = self.closed(Proposition::IntegerMathLessOrEqual(
            left_limit.clone(),
            right_limit.clone(),
        ))?;
        let step = node(
            Proposition::IntegerMathLessOrEqual(left.clone(), right_limit.clone()),
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(upper),
                middle_less_or_equal_right: Box::new(bridge),
            },
        );
        Some(node(
            goal.clone(),
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(step),
                middle_less_or_equal_right: Box::new(lower),
            },
        ))
    }

    /// Prove an integer comparison from a known order or bounds on its operands.
    fn compare(
        &mut self,
        left: &ScalarTerm,
        right: &ScalarTerm,
        strict: bool,
    ) -> Result<ProofNode, Unproven> {
        let ScalarType::Integer(integer) = left.scalar_type() else {
            return Err("order of non-integers");
        };
        if !strict
            && let Some(fact) = self.exact_fact(&Proposition::LessThan(left.clone(), right.clone()))
        {
            return weaken(self.facts[fact].proof(&self.facts));
        }
        if let (ScalarTerm::Value { .. }, ScalarTerm::Value { .. }) = (left, right)
            && let Some(proof) = self
                .relation(left, right, strict)
                .or_else(|| (!strict).then(|| self.successor(left, right)).flatten())
        {
            return Ok(proof);
        }
        if let Some((_, _, divisor)) = left.integer_remainder_operands()
            && divisor == right
            && let Some((_, proof)) = self.remainder_below_divisor(None, left, left)
        {
            return if strict { Ok(proof) } else { weaken(proof) };
        }
        if let Some(proof) = self.saturating_subtract_order(left, right, strict) {
            return Ok(proof);
        }
        let carrier_order = if strict {
            Proposition::LessThan(left.clone(), right.clone())
        } else {
            Proposition::LessOrEqual(left.clone(), right.clone())
        };
        if let Some(proof) = self
            .positive_difference(&carrier_order)
            .or_else(|| self.sum_order(&carrier_order))
        {
            return Ok(proof);
        }
        let left_side = self.comparison_side(left, Direction::Upper)?;
        let right_side = self.comparison_side(right, Direction::Lower)?;
        let ordered = integer
            .compare(left_side.literal, right_side.literal)
            .ok_or("bounds are not comparable")?;
        if ordered == Ordering::Greater || (strict && ordered == Ordering::Equal) {
            return Err("the bounds do not decide the comparison");
        }
        let literal = |value| ScalarTerm::integer(integer, value).map_err(|_| "bound literal");
        let middle_left = match left {
            ScalarTerm::Integer { .. } => left.clone(),
            _ => literal(left_side.literal)?,
        };
        let middle_right = match right {
            ScalarTerm::Integer { .. } => right.clone(),
            _ => literal(right_side.literal)?,
        };
        let middle = if strict {
            Proposition::LessThan(middle_left, middle_right)
        } else {
            Proposition::LessOrEqual(middle_left, middle_right)
        };
        let mut proof = self
            .closed(middle)
            .ok_or("the closed bound relation does not hold")?;
        if let Some(left_proof) = self.comparison_proof(left, Direction::Upper, left_side)? {
            proof = chain(left_proof, proof)?;
        }
        if let Some(right_proof) = self.comparison_proof(right, Direction::Lower, right_side)? {
            proof = chain(proof, right_proof)?;
        }
        Ok(proof)
    }

    /// `left <= right` where the result equations define `left` as one more
    /// than a value below `right`, such as a loop's `position + 1` against a
    /// length the guard `position < length` bounded. Neither side has a
    /// literal bound, so interval reasoning cannot see it. Integers are
    /// discrete: `a < b` gives `a + 1 <= b` (`IntegerOrderDiscreteness`), and
    /// the same equations carry that back onto `left`
    /// (`ValueEqualityTransport`). The kernel recomputes the unfolded form.
    fn successor(&mut self, left: &ScalarTerm, right: &ScalarTerm) -> Option<ProofNode> {
        if self.equations.is_empty() {
            return None;
        }
        let goal = Proposition::LessOrEqual(left.clone(), right.clone());
        crate::checking::known::Known::transport_inputs(
            &self.facts,
            &self.equations,
            &goal,
            &self.definitions,
            &mut self.transport,
        );
        let selected: Vec<usize> = self
            .transport
            .selected
            .iter()
            .map(|&index| self.equations[index])
            .collect();
        // A previously computed addend is a usable order endpoint. Keep it
        // intact before trying the full denotation (needed for aliases).
        let immediate: Vec<_> = selected.iter().copied().filter(|&index| {
            matches!(&self.facts[index].proposition, Proposition::Equal(result, operand)
                if result == left || matches!(operand, ScalarTerm::Integer { .. } | ScalarTerm::Value { .. }))
        }).collect();
        if immediate.len() != selected.len()
            && let Some(proof) = self.successor_via(&goal, &immediate)
        {
            return Some(proof);
        }
        self.successor_via(&goal, &selected)
    }

    fn successor_via(&mut self, goal: &Proposition, selected: &[usize]) -> Option<ProofNode> {
        // The goal with its values unfolded, such as `a +exact 1 <= b`, and
        // that order read over the mathematical integers.
        let unfolded = check_value_equality_denotation(
            self.context,
            goal,
            selected.iter().map(|&index| &self.facts[index].proposition),
        )
        .ok()?;
        let lifted = lift_exact_integer_order(&unfolded)?;
        let Proposition::IntegerMathLessOrEqual(IntegerMathTerm::Add(below, one), above) = &lifted
        else {
            return None;
        };
        let (
            IntegerMathTerm::MathValue {
                source_type,
                value: below,
            },
            IntegerMathTerm::MathValue {
                source_type: other,
                value: above,
            },
        ) = (below.as_ref(), above)
        else {
            return None;
        };
        if source_type != other || **one != IntegerMathTerm::literal(IntegerValue::Unsigned(1)) {
            return None;
        }
        let scalar_type = ScalarType::Integer(*source_type);
        let strict = self.relation(
            &ScalarTerm::value(*below, scalar_type),
            &ScalarTerm::value(*above, scalar_type),
            true,
        )?;
        let upper = IntegerMathTerm::MathValue {
            source_type: *source_type,
            value: *above,
        };
        let discrete = node(
            lifted,
            ProofRule::IntegerOrderDiscreteness {
                relation: Box::new(strict),
            },
        );
        // Substituting the bound for itself changes nothing; the rule lets
        // the mathematical order conclude in the carrier spelling transport
        // unfolds the goal to.
        let unchanged = primitive(
            Proposition::IntegerMathEqual(upper.clone(), upper),
            PrimitiveJudgment::ReflexiveEquality,
        );
        let carrier = node(
            unfolded,
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(discrete),
                equality: Box::new(unchanged),
                endpoint: 1,
            },
        );
        let equalities = selected
            .iter()
            .map(|&index| self.facts[index].proof(&self.facts))
            .collect();
        Some(node(
            goal.clone(),
            ProofRule::ValueEqualityTransport {
                premise: Box::new(carrier),
                equalities,
            },
        ))
    }

    /// Compare results from their checked definitions. In the strict form
    /// a strict coordinate is useful only above the larger result's clamp.
    fn saturating_subtract_order(
        &mut self,
        smaller: &ScalarTerm,
        larger: &ScalarTerm,
        strict: bool,
    ) -> Option<ProofNode> {
        let definition = |subject: &ScalarTerm| {
            let ScalarTerm::Value { id, .. } = subject else {
                return None;
            };
            let index = self.definitions.get(*id)?.as_ref()?;
            self.facts.get(self.equations[*index])
        };
        let smaller_fact = definition(smaller);
        let larger_fact = definition(larger);
        let saturated = |fact: Option<&crate::checking::known::Fact>, subject: &ScalarTerm| {
            matches!(subject, ScalarTerm::SaturatingIntegerSubtract { .. })
                || fact.is_some_and(|fact| {
                    matches!(
                        &fact.proposition,
                        Proposition::Equal(_, ScalarTerm::SaturatingIntegerSubtract { .. })
                    )
                })
        };
        if !saturated(smaller_fact, smaller) && !saturated(larger_fact, larger) {
            return None;
        }
        // Even when a value is itself a formed difference, its reflexive
        // equation can name it as a whole: (end - cursor) -| 0 is compared
        // with that exact value without needing cursor itself to be zero.
        let equations = |fact: Option<&crate::checking::known::Fact>, subject: &ScalarTerm| {
            let definition = fact
                .filter(|fact| proof_admission::unsigned_difference(&fact.proposition).is_some())
                .map(|fact| fact.proof(&self.facts));
            [
                definition,
                self.reflexive(Proposition::Equal(subject.clone(), subject.clone())),
            ]
        };
        let smaller = equations(smaller_fact, smaller);
        let larger = equations(larger_fact, larger);
        for after in smaller.iter().flatten() {
            for before in larger.iter().flatten() {
                if let Some(proof) = self.unsigned_difference_order(after, before, strict) {
                    return Some(proof);
                }
            }
        }
        None
    }

    fn unsigned_difference_order(
        &mut self,
        smaller_equation: &ProofNode,
        larger_equation: &ProofNode,
        strict: bool,
    ) -> Option<ProofNode> {
        let after = proof_admission::unsigned_difference(&smaller_equation.conclusion)?;
        let before = proof_admission::unsigned_difference(&larger_equation.conclusion)?;
        if !after.saturated && !before.saturated {
            return None;
        }
        let smaller = after.result;
        let larger = before.result;
        let larger_minuend = before.minuend.clone();
        let larger_subtrahend = before.subtrahend;
        let minuends = (after.minuend.clone(), larger_minuend.clone());
        let subtrahends = (larger_subtrahend.clone(), after.subtrahend);
        let mut proofs;
        let conclusion = if strict {
            let positive = self
                .prove(&Proposition::LessThan(larger_subtrahend, larger_minuend))
                .ok()?;
            proofs = self
                .prove(&Proposition::LessThan(
                    minuends.0.clone(),
                    minuends.1.clone(),
                ))
                .and_then(|first| {
                    self.prove(&Proposition::LessOrEqual(
                        subtrahends.0.clone(),
                        subtrahends.1.clone(),
                    ))
                    .map(|second| vec![first, second])
                })
                .or_else(|_| {
                    let second =
                        self.prove(&Proposition::LessThan(subtrahends.0, subtrahends.1))?;
                    let first = self.prove(&Proposition::LessOrEqual(minuends.0, minuends.1))?;
                    Ok::<_, Unproven>(vec![first, second])
                })
                .ok()?;
            proofs.push(positive);
            Proposition::LessThan(smaller.clone(), larger.clone())
        } else {
            proofs = vec![
                self.prove(&Proposition::LessOrEqual(minuends.0, minuends.1))
                    .ok()?,
                self.prove(&Proposition::LessOrEqual(subtrahends.0, subtrahends.1))
                    .ok()?,
            ];
            Proposition::LessOrEqual(smaller.clone(), larger.clone())
        };
        let order = node(
            Proposition::Conjunction(
                proofs
                    .iter()
                    .map(|proof| proof.conclusion.clone())
                    .collect(),
            ),
            ProofRule::ConjunctionIntroduction(proofs),
        );
        Some(node(
            conclusion,
            ProofRule::IntegerSubtractAntitone {
                smaller: Box::new(smaller_equation.clone()),
                larger: Box::new(larger_equation.clone()),
                order: Box::new(order),
            },
        ))
    }

    /// `from <= to` (or `<` when `strict`) between two values through order
    /// relations among values, with no literal endpoint: a breadth-first
    /// search whose steps are hypotheses relating two values and exact add or
    /// subtract results ordered against their left operand. Each step is a
    /// node the kernel checks; the path is chained by order transitivity.
    fn relation(&mut self, from: &ScalarTerm, to: &ScalarTerm, strict: bool) -> Option<ProofNode> {
        let mut visited = self.bounds.like([false; 2]);
        let mut queue: std::collections::VecDeque<(ScalarTerm, ProofNode, bool)> =
            self.steps(from, to, strict).into_iter().collect();
        while let Some((at, proof, at_strict)) = queue.pop_front() {
            if let ScalarTerm::Value { id, .. } = &at {
                let seen = visited.get_mut(*id)?;
                if seen[usize::from(at_strict)] {
                    continue;
                }
                seen[usize::from(at_strict)] = true;
            } else if &at != to {
                continue;
            }
            if &at == to {
                if at_strict || !strict {
                    return Some(if at_strict && !strict {
                        weaken(proof).ok()?
                    } else {
                        proof
                    });
                }
                continue;
            }
            for (next, step, step_strict) in self.steps(&at, to, strict && !at_strict) {
                if let Ok(chained) = chain(proof.clone(), step) {
                    queue.push_back((next, chained, at_strict || step_strict));
                }
            }
        }
        None
    }

    /// The order steps leaving `at`: `(next, proof of at R next, strict)`.
    fn steps(
        &mut self,
        at: &ScalarTerm,
        to: &ScalarTerm,
        strict: bool,
    ) -> Vec<(ScalarTerm, ProofNode, bool)> {
        let mut steps = Vec::new();
        let ScalarTerm::Value { id, .. } = at else {
            return steps;
        };
        // Every matching case names `at`, either as an endpoint or within a
        // compound term. Reuse the prepared ordinal index, including its
        // opaque-term fallback, without changing which fact is tried first.
        let mut cursor = BoundFactCursor::default();
        while let Some(index) = self.bound_facts.next(*id, &mut cursor) {
            let proposition = self.facts[index].proposition.clone();
            let axiom = self.facts[index].axiom;
            match &proposition {
                Proposition::LessOrEqual(left, right) | Proposition::LessThan(left, right)
                    if left == at && (matches!(right, ScalarTerm::Value { .. }) || right == to) =>
                {
                    // A compound endpoint can finish only this exact query;
                    // it never becomes another graph vertex.
                    let strict = matches!(proposition, Proposition::LessThan(..));
                    steps.push((right.clone(), self.facts[index].proof(&self.facts), strict));
                }
                Proposition::Equal(left, right)
                    if left != right
                        && ((left == at
                            && (matches!(right, ScalarTerm::Value { .. }) || right == to))
                            || (right == at
                                && (matches!(left, ScalarTerm::Value { .. }) || left == to))) =>
                {
                    let proof = self.facts[index].proof(&self.facts);
                    let oriented = if left == at { Ok(proof) } else { flip(proof) };
                    if let Ok(weakened) = oriented.and_then(weaken) {
                        let next = if left == at { right } else { left };
                        steps.push((next.clone(), weakened, false));
                    }
                }
                // `a < a + d` and `a - d < a` when `0 < d` (or `<=` when `0 <= d`).
                // An arrival can prove the same result equation as an operation
                // axiom. Either equation warrants the checked order step.
                Proposition::Equal(
                    output,
                    sum @ ScalarTerm::ExactIntegerAdd {
                        scalar_type,
                        left,
                        right,
                    },
                ) if matches!(output, ScalarTerm::Value { .. }) && left.as_ref() == at => {
                    if let Some(positive) = self.positive(*scalar_type, right) {
                        let conclusion = Proposition::LessThan(at.clone(), output.clone());
                        steps.push((
                            output.clone(),
                            node(
                                conclusion,
                                ProofRule::IntegerAddOrder {
                                    sum: Box::new(self.facts[index].proof(&self.facts)),
                                    positive: Box::new(positive),
                                },
                            ),
                            true,
                        ));
                    } else if let Some(nonnegative) = self.nonnegative(*scalar_type, right)
                        && let Some(at_to_sum) =
                            self.order_from_nonnegative_offset(*scalar_type, nonnegative, at, sum)
                        && let Ok(sum_to_output) =
                            flip(self.facts[index].proof(&self.facts)).and_then(weaken)
                        && let Ok(chained) = chain(at_to_sum, sum_to_output)
                    {
                        steps.push((output.clone(), chained, false));
                    }
                }
                Proposition::Equal(
                    output,
                    difference @ ScalarTerm::ExactIntegerSubtract {
                        scalar_type,
                        left,
                        right,
                    },
                ) if output == at => {
                    if let Some(positive) = self.positive(*scalar_type, right) {
                        let conclusion =
                            Proposition::LessThan(output.clone(), left.as_ref().clone());
                        steps.push((
                            left.as_ref().clone(),
                            node(
                                conclusion,
                                ProofRule::IntegerSubtractOrder {
                                    difference: Box::new(self.facts[index].proof(&self.facts)),
                                    positive: Box::new(positive),
                                },
                            ),
                            true,
                        ));
                    } else if matches!(left.as_ref(), ScalarTerm::Value { .. })
                        && let Some(nonnegative) = self.nonnegative(*scalar_type, right)
                        && let Some(diff_to_left) = self.order_from_nonnegative_offset(
                            *scalar_type,
                            nonnegative,
                            difference,
                            left,
                        )
                        && let Ok(output_to_diff) = weaken(self.facts[index].proof(&self.facts))
                        && let Ok(chained) = chain(output_to_diff, diff_to_left)
                    {
                        steps.push((left.as_ref().clone(), chained, false));
                    }
                }
                Proposition::Equal(output, sum @ ScalarTerm::ExactIntegerAdd { .. })
                    if output == at =>
                {
                    steps.extend(self.interior_sum_edges(output, sum));
                }
                // `a / d <= a` when `0 <= a` and `0 < d`.
                Proposition::Equal(output, expression @ ScalarTerm::ExactIntegerDivide { .. })
                    if output == at =>
                {
                    if let Some(axiom) = axiom {
                        let relation = strict
                            .then(|| self.quotient_order(axiom, output, expression, true))
                            .flatten()
                            .or_else(|| self.quotient_order(axiom, output, expression, false));
                        if let Some((dividend, proof)) = relation {
                            let strict = matches!(proof.conclusion, Proposition::LessThan(..));
                            steps.push((dividend, proof, strict));
                        }
                    }
                }
                // `a % d < d` for an unsigned remainder when `0 < d`.
                Proposition::Equal(output, expression)
                    if output == at && expression.integer_remainder_operands().is_some() =>
                {
                    if let Some(axiom) = axiom
                        && let Some((divisor, below)) =
                            self.remainder_below_divisor(Some(axiom), output, expression)
                    {
                        steps.push((divisor, below, true));
                    }
                }
                // Guards and contracts often state arithmetic directly, such as
                // `index + 1 < out.length` or `index <= out.length - 1`, without an
                // SSA value naming the sum or difference in the callee.
                Proposition::LessOrEqual(left, right) | Proposition::LessThan(left, right)
                    if !is_bare_scalar(left) || !is_bare_scalar(right) =>
                {
                    if let Some(step) = self.compound_order_step(
                        at,
                        left,
                        right,
                        self.facts[index].proof(&self.facts),
                    ) {
                        steps.push(step);
                    }
                }
                Proposition::Equal(left, right)
                    if left != right && (!is_bare_scalar(left) || !is_bare_scalar(right)) =>
                {
                    if term_mentions(left, at)
                        && let Ok(weakened) = weaken(self.facts[index].proof(&self.facts))
                        && let Some(step) = self.compound_order_step(at, left, right, weakened)
                    {
                        steps.push(step);
                    }
                    if term_mentions(right, at)
                        && let Ok(weakened) =
                            flip(self.facts[index].proof(&self.facts)).and_then(weaken)
                        && let Some(step) = self.compound_order_step(at, right, left, weakened)
                    {
                        steps.push(step);
                    }
                }
                _ => {}
            }
        }
        steps
    }

    /// A proof of `0 < term`.
    fn positive(&mut self, integer: IntegerType, term: &ScalarTerm) -> Option<ProofNode> {
        #[cfg(test)]
        {
            self.activity.sign_queries += 1;
        }
        let zero = ScalarTerm::integer(integer, integer.zero()).ok()?;
        self.prove(&Proposition::LessThan(zero, term.clone())).ok()
    }

    /// A proof of `0 <= term`.
    fn nonnegative(&mut self, integer: IntegerType, term: &ScalarTerm) -> Option<ProofNode> {
        #[cfg(test)]
        {
            self.activity.sign_queries += 1;
        }
        let zero = ScalarTerm::integer(integer, integer.zero()).ok()?;
        self.prove(&Proposition::LessOrEqual(zero, term.clone()))
            .ok()
    }

    /// Proves `smaller <= larger` from `nonnegative: 0 <= offset` when
    /// `offset + smaller == larger` over mathematical integers (covering
    /// `a <= a + d`, `a <= d + a`, and `b - d <= b`).
    fn order_from_nonnegative_offset(
        &self,
        integer: IntegerType,
        nonnegative: ProofNode,
        smaller: &ScalarTerm,
        larger: &ScalarTerm,
    ) -> Option<ProofNode> {
        if integer.carrier() != IntegerCarrier::Fixed || integer.is_address() {
            return None;
        }
        let Proposition::LessOrEqual(zero, offset) = &nonnegative.conclusion else {
            return None;
        };
        let zero_math = IntegerMathTerm::embed(zero.clone())?;
        let offset_math = IntegerMathTerm::embed(offset.clone())?;
        let embedded_nonneg =
            Proposition::IntegerMathLessOrEqual(zero_math.clone(), offset_math.clone());
        let goal = Proposition::LessOrEqual(smaller.clone(), larger.clone());
        let Some(Proposition::IntegerMathLessOrEqual(smaller_math, larger_math)) =
            lift_exact_integer_order(&goal)
        else {
            return None;
        };
        let zero_refl = self.reflexive(Proposition::Equal(zero.clone(), zero.clone()))?;
        let math_nonneg = node(
            embedded_nonneg,
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(nonnegative),
                equality: Box::new(zero_refl),
                endpoint: 0,
            },
        );
        let translated_left =
            IntegerMathTerm::Add(Box::new(zero_math), Box::new(smaller_math.clone()));
        let translated_right =
            IntegerMathTerm::Add(Box::new(offset_math), Box::new(smaller_math.clone()));
        let translated = node(
            Proposition::IntegerMathLessOrEqual(translated_left.clone(), translated_right.clone()),
            ProofRule::IntegerMathOrderTranslation {
                relation: Box::new(math_nonneg),
                addend: smaller_math.clone(),
            },
        );
        let left_eq = self.closed(sorted_math_equal(translated_left, smaller_math.clone()))?;
        let after_left = node(
            Proposition::IntegerMathLessOrEqual(smaller_math, translated_right.clone()),
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(translated),
                equality: Box::new(left_eq),
                endpoint: 0,
            },
        );
        let right_eq = self.closed(sorted_math_equal(translated_right, larger_math))?;
        Some(node(
            goal,
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(after_left),
                equality: Box::new(right_eq),
                endpoint: 1,
            },
        ))
    }

    /// One step `branch < sum` (when `0 < other`) or `branch <= sum` (when
    /// `0 <= other`) inside `sum = ExactIntegerAdd`.
    fn add_operand_order(
        &mut self,
        integer: IntegerType,
        branch: &ScalarTerm,
        other: &ScalarTerm,
        sum: &ScalarTerm,
        on_left: bool,
    ) -> Option<ProofNode> {
        if integer.carrier() != IntegerCarrier::Fixed || integer.is_address() {
            return None;
        }
        if let Some(positive) = self.positive(integer, other) {
            if on_left {
                let sum_eq = self.reflexive(Proposition::Equal(sum.clone(), sum.clone()))?;
                return Some(node(
                    Proposition::LessThan(branch.clone(), sum.clone()),
                    ProofRule::IntegerAddOrder {
                        sum: Box::new(sum_eq),
                        positive: Box::new(positive),
                    },
                ));
            }
            let commuted =
                ScalarTerm::exact_integer_add(integer, branch.clone(), other.clone()).ok()?;
            let commuted_refl =
                self.reflexive(Proposition::Equal(commuted.clone(), commuted.clone()))?;
            let uncommuted_order = node(
                Proposition::LessThan(branch.clone(), commuted.clone()),
                ProofRule::IntegerAddOrder {
                    sum: Box::new(commuted_refl),
                    positive: Box::new(positive),
                },
            );
            let commuted_eq = self.closed(Proposition::Equal(commuted, sum.clone()))?;
            return Some(node(
                Proposition::LessThan(branch.clone(), sum.clone()),
                ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(uncommuted_order),
                    equality: Box::new(commuted_eq),
                    endpoint: 1,
                },
            ));
        }
        let nonnegative = self.nonnegative(integer, other)?;
        self.order_from_nonnegative_offset(integer, nonnegative, branch, sum)
    }

    /// Proves `sub < term` or `sub <= term` when `term` adds positive or
    /// nonnegative exact offsets onto `sub`.
    fn below_compound(&mut self, sub: &ScalarTerm, term: &ScalarTerm) -> Option<ProofNode> {
        if term == sub || !term_mentions(term, sub) {
            return None;
        }
        let ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } = term
        else {
            return None;
        };
        if scalar_type.carrier() != IntegerCarrier::Fixed || scalar_type.is_address() {
            return None;
        }
        let mut best = None;
        for (branch, other, on_left) in [
            (left.as_ref(), right.as_ref(), true),
            (right.as_ref(), left.as_ref(), false),
        ] {
            if !term_mentions(branch, sub) {
                continue;
            }
            let Some(branch_to_term) =
                self.add_operand_order(*scalar_type, branch, other, term, on_left)
            else {
                continue;
            };
            let candidate = if branch == sub {
                Some(branch_to_term)
            } else {
                self.below_compound(sub, branch)
                    .and_then(|inner| chain(inner, branch_to_term).ok())
            };
            let Some(candidate) = candidate else {
                continue;
            };
            if matches!(candidate.conclusion, Proposition::LessThan(..)) {
                return Some(candidate);
            }
            if best.is_none() {
                best = Some(candidate);
            }
        }
        best
    }

    /// Reject an irrelevant subtraction chain before proving any offset signs.
    /// A failed deeper sign proof can leave an intermediate left term as the
    /// result, so every left term remains a possible endpoint here.
    fn upper_subterm_can_reach(
        mut term: &ScalarTerm,
        wanted: impl Fn(&ScalarTerm) -> bool,
    ) -> bool {
        while let ScalarTerm::ExactIntegerSubtract {
            scalar_type, left, ..
        } = term
        {
            if scalar_type.carrier() != IntegerCarrier::Fixed || scalar_type.is_address() {
                return false;
            }
            if wanted(left) {
                return true;
            }
            term = left;
        }
        false
    }

    /// Follows exact subtractions of positive or nonnegative offsets upward to
    /// the underlying minuend: `(sup, proof of term < sup or term <= sup)`.
    fn upper_subterm(&mut self, term: &ScalarTerm) -> Option<(ScalarTerm, ProofNode)> {
        let ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } = term
        else {
            return None;
        };
        if scalar_type.carrier() != IntegerCarrier::Fixed || scalar_type.is_address() {
            return None;
        }
        let step_up = if let Some(positive) = self.positive(*scalar_type, right) {
            let diff_eq = self.reflexive(Proposition::Equal(term.clone(), term.clone()))?;
            node(
                Proposition::LessThan(term.clone(), left.as_ref().clone()),
                ProofRule::IntegerSubtractOrder {
                    difference: Box::new(diff_eq),
                    positive: Box::new(positive),
                },
            )
        } else {
            let nonnegative = self.nonnegative(*scalar_type, right)?;
            self.order_from_nonnegative_offset(*scalar_type, nonnegative, term, left)?
        };
        if let Some((root, further)) = self.upper_subterm(left)
            && let Ok(chained) = chain(step_up.clone(), further)
        {
            return Some((root, chained));
        }
        Some((left.as_ref().clone(), step_up))
    }

    /// Extracts a value-to-value step `(next, proof of at R next, strict)` from
    /// an order `left <= right` or `left < right` whose endpoints may be
    /// compound exact sums or differences.
    fn compound_order_step(
        &mut self,
        at: &ScalarTerm,
        left: &ScalarTerm,
        right: &ScalarTerm,
        order_proof: ProofNode,
    ) -> Option<(ScalarTerm, ProofNode, bool)> {
        let relevant = if matches!(right, ScalarTerm::Value { .. }) {
            right != at
        } else {
            Self::upper_subterm_can_reach(right, |candidate| {
                matches!(candidate, ScalarTerm::Value { .. }) && candidate != at
            })
        };
        if !relevant {
            return None;
        }
        let at_to_left = if left == at {
            None
        } else {
            Some(self.below_compound(at, left)?)
        };
        let (next, right_to_next) = if matches!(right, ScalarTerm::Value { .. }) {
            (right.clone(), None)
        } else {
            let (sup, step_out) = self.upper_subterm(right)?;
            if !matches!(sup, ScalarTerm::Value { .. }) {
                return None;
            }
            (sup, Some(step_out))
        };
        if &next == at {
            return None;
        }
        let mut proof = match at_to_left {
            Some(step_in) => chain(step_in, order_proof).ok()?,
            None => order_proof,
        };
        if let Some(step_out) = right_to_next {
            proof = chain(proof, step_out).ok()?;
        }
        let strict = matches!(proof.conclusion, Proposition::LessThan(..));
        Some((next, proof, strict))
    }

    /// Settle each endpoint in order, leaving value proofs in the cache.
    fn comparison_side(
        &mut self,
        term: &ScalarTerm,
        direction: Direction,
    ) -> Result<ComparisonSide, Unproven> {
        match term {
            ScalarTerm::Integer { value, .. } => Ok(ComparisonSide {
                literal: *value,
                expression_proof: None,
            }),
            ScalarTerm::Value { id, .. } => Ok(ComparisonSide {
                literal: self
                    .bound_literal(*id, direction)
                    .ok_or("value has no bound")?,
                expression_proof: None,
            }),
            _ => {
                let integer = term
                    .scalar_type()
                    .as_integer()
                    .ok_or("comparison term outside the scalar subset")?;
                // A result's carrier bound can be looser than the arithmetic
                // image, while a result's local facts can sharpen that image.
                let image = self.bound_from_arithmetic_image(term, direction);
                let definition = self.bound_on_definition(term, integer, direction);
                let multiple = self.multiple_remainder_bound(term, None, direction);
                let best_extra = match (definition, multiple) {
                    (Some(left), Some(right)) => Some(
                        if direction.prefers(integer.compare(right.literal, left.literal)) {
                            right
                        } else {
                            left
                        },
                    ),
                    (left, right) => left.or(right),
                };
                let bound = match (image, best_extra) {
                    (Ok(image), Some(definition))
                        if direction
                            .prefers(integer.compare(definition.literal, image.literal)) =>
                    {
                        definition
                    }
                    (Ok(image), _) => image,
                    (Err(_), Some(definition)) => definition,
                    (Err(reason), None) => return Err(reason),
                };
                Ok(ComparisonSide {
                    literal: bound.literal,
                    expression_proof: Some(bound.proof),
                })
            }
        }
    }

    fn comparison_proof(
        &mut self,
        term: &ScalarTerm,
        direction: Direction,
        side: ComparisonSide,
    ) -> Result<Option<ProofNode>, Unproven> {
        match term {
            // The endpoint search either completed the slot or reused one
            // owned by an outer search. Neither can change while this
            // comparison settles the other endpoint.
            ScalarTerm::Value { id, .. } => {
                let bound = self
                    .copy_cached_bound(*id, direction)
                    .ok_or("value has no bound")?;
                debug_assert_eq!(side.literal, bound.literal);
                Ok(Some(bound.proof))
            }
            _ => Ok(side.expression_proof),
        }
    }

    fn bound_from_arithmetic_image(
        &mut self,
        term: &ScalarTerm,
        direction: Direction,
    ) -> Result<Bound, Unproven> {
        let Some(Proposition::IntegerMathLessOrEqual(expression, _)) =
            lift_exact_integer_order(&Proposition::LessOrEqual(term.clone(), term.clone()))
        else {
            return Err("comparison term outside the scalar subset");
        };
        let relation = self.arithmetic_image(&expression, direction)?;
        let equality = self
            .reflexive(Proposition::IntegerMathEqual(
                expression.clone(),
                expression,
            ))
            .ok_or("the kernel refuses the expression identity")?;
        Self::carrier_bound_from_image(term, relation, equality, direction)
            .ok_or("the image bound is outside the comparison carrier")
    }

    /// A predicate can unfold an operation that already produced a scalar
    /// here. Each value with that exact definition can supply a bound on the
    /// unfolded term, so use the tightest one available in this context.
    fn bound_on_definition(
        &mut self,
        term: &ScalarTerm,
        integer: IntegerType,
        direction: Direction,
    ) -> Option<Bound> {
        let (index, bound) = self
            .axioms
            .iter()
            .enumerate()
            .filter_map(|(index, axiom)| {
                let value = match axiom {
                    Proposition::Equal(
                        ScalarTerm::Value {
                            id,
                            scalar_type: ScalarType::Integer(result_type),
                        },
                        definition,
                    ) if definition == term && *result_type == integer => *id,
                    _ => return None,
                };
                self.bound(value, direction).map(|bound| (index, bound))
            })
            .reduce(|best, next| {
                let (_, best_bound) = &best;
                let (_, next_bound) = &next;
                if direction.prefers(integer.compare(next_bound.literal, best_bound.literal)) {
                    next
                } else {
                    best
                }
            })?;
        let goal = Self::oriented(
            term,
            ScalarTerm::integer(integer, bound.literal).ok()?,
            direction,
        );
        let equality = node(
            self.axioms[index].clone(),
            ProofRule::SemanticAxiom { index },
        );
        Some(Bound {
            literal: bound.literal,
            proof: node(
                goal,
                ProofRule::ValueEqualityTransport {
                    premise: Box::new(bound.proof),
                    equalities: vec![equality],
                },
            ),
        })
    }

    /// The proof that a bound evidence part needs: `Truth` for a literal
    /// operand, the value's bound otherwise.
    fn evidence(&mut self, term: &ScalarTerm, direction: Direction) -> Result<ProofNode, Unproven> {
        match term {
            ScalarTerm::Integer { .. } => {
                Ok(primitive(Proposition::Truth, PrimitiveJudgment::Truth))
            }
            ScalarTerm::Value { id, .. } => self
                .bound(*id, direction)
                .map(|bound| bound.proof)
                .ok_or("operand has no bound"),
            _ => Err("operand outside the scalar subset"),
        }
    }

    /// The best bound on `value` in `direction`, memoized for this goal set.
    fn bound(&mut self, value: ValueId, direction: Direction) -> Option<Bound> {
        self.bound_literal(value, direction)?;
        self.copy_cached_bound(value, direction)
    }

    fn copy_cached_bound(&mut self, value: ValueId, direction: Direction) -> Option<Bound> {
        #[cfg(test)]
        {
            self.activity.bound_certificate_copies += 1;
        }
        match &self.bounds.get(value)?[direction.index()] {
            Slot::Done(bound) | Slot::InProgress(bound) => bound.clone(),
            Slot::Unknown => None,
        }
    }

    /// Settle the same search without copying its certificate. A relation
    /// needs the endpoint first; evidence matters only if that endpoint
    /// tightens the best bound already retained by its caller.
    fn bound_literal(&mut self, value: ValueId, direction: Direction) -> Option<IntegerValue> {
        let slot = self.bounds.get(value)?.get(direction.index())?;
        match slot {
            Slot::Done(bound) => return bound.as_ref().map(|bound| bound.literal),
            // A recursive candidate may reuse an already completed proof,
            // never the candidate currently being derived. With no such
            // proof this relation cycle contributes nothing.
            Slot::InProgress(bound) => {
                #[cfg(test)]
                if bound.is_some() {
                    self.activity.in_progress_bound_reuses += 1;
                }
                return bound.as_ref().map(|bound| bound.literal);
            }
            Slot::Unknown => {}
        }
        self.bounds.get_mut(value)?[direction.index()] = Slot::InProgress(None);
        let best = self.search(value, direction);
        let literal = best.as_ref().map(|bound| bound.literal);
        self.bounds.get_mut(value)?[direction.index()] = Slot::Done(best);
        literal
    }

    fn search(&mut self, value: ValueId, direction: Direction) -> Option<Bound> {
        let ScalarType::Integer(integer) = self.values.scalar_type(value)? else {
            return None;
        };
        #[cfg(test)]
        {
            self.activity.bound_value_searches += 1;
        }
        let subject = ScalarTerm::value(value, ScalarType::Integer(integer));
        let mut best = self.carrier(&subject, integer, direction);
        // Direct endpoints require no recursive search. Make their proofs
        // available before following relations or arithmetic definitions:
        // a wrapping step may need precisely this value's headroom.
        let mut candidates = BoundFactCursor::default();
        while let Some(index) = self.bound_facts.next(value, &mut candidates) {
            #[cfg(test)]
            {
                self.activity.bound_fact_visits += 1;
            }
            if let Some(next) = self.literal_bound_from_fact(
                index,
                &subject,
                integer,
                direction,
                best.as_ref().map(|bound| bound.literal),
            ) && best.as_ref().is_none_or(|current| {
                direction.prefers(integer.compare(next.literal, current.literal))
            }) {
                best = Some(next);
            }
        }
        self.bounds.get_mut(value)?[direction.index()] = Slot::InProgress(best.clone());
        let mut candidates = BoundFactCursor::default();
        while let Some(index) = self.bound_facts.next(value, &mut candidates) {
            #[cfg(test)]
            {
                self.activity.bound_fact_visits += 1;
            }
            let current = best.as_ref().map(|bound| bound.literal);
            if let Some(next) = self.bound_from_fact(index, &subject, integer, direction, current)
                && best.as_ref().is_none_or(|current| {
                    direction.prefers(integer.compare(next.literal, current.literal))
                })
            {
                best = Some(next);
                self.bounds.get_mut(value)?[direction.index()] = Slot::InProgress(best.clone());
            }
        }
        best
    }

    fn oriented(subject: &ScalarTerm, literal: ScalarTerm, direction: Direction) -> Proposition {
        match direction {
            Direction::Upper => Proposition::LessOrEqual(subject.clone(), literal),
            Direction::Lower => Proposition::LessOrEqual(literal, subject.clone()),
        }
    }

    fn carrier(
        &self,
        subject: &ScalarTerm,
        integer: IntegerType,
        direction: Direction,
    ) -> Option<Bound> {
        let literal = match direction {
            Direction::Upper => integer.maximum_value()?,
            Direction::Lower => integer.minimum_value()?,
        };
        let conclusion = Self::oriented(
            subject,
            ScalarTerm::integer(integer, literal).ok()?,
            direction,
        );
        decide_primitive(
            self.context,
            &conclusion,
            PrimitiveJudgment::IntegerCarrierBound,
        )
        .ok()?;
        Some(Bound {
            literal,
            proof: primitive(conclusion, PrimitiveJudgment::IntegerCarrierBound),
        })
    }

    /// `proof` of `subject R literal` (`literal R subject` for a lower bound)
    /// restated so that its literal side is the plain literal the bound names.
    /// A fact may bound a value by a closed term, such as `(20 + 11) * 3 + 1`
    /// once value-equality transport has substituted every operand, and the
    /// bound's literal is that term's value. Callers chain the proof through
    /// the literal, and the kernel's order rules match the middle endpoint
    /// exactly, so the proof must end at that literal: the closed relation
    /// between the term and its value is one more chained step.
    fn literal_endpoint(
        &self,
        proof: ProofNode,
        literal: &ScalarTerm,
        direction: Direction,
    ) -> Option<(IntegerValue, ProofNode)> {
        let (integer, value) = literal.integer_value()?;
        if matches!(literal, ScalarTerm::Integer { .. }) {
            return Some((value, proof));
        }
        let plain = ScalarTerm::integer(integer, value).ok()?;
        let restated = match direction {
            Direction::Upper => chain(
                proof,
                self.closed(Proposition::LessOrEqual(literal.clone(), plain))?,
            ),
            Direction::Lower => chain(
                self.closed(Proposition::LessOrEqual(plain, literal.clone()))?,
                proof,
            ),
        };
        Some((value, restated.ok()?))
    }

    /// A fact with a closed endpoint supplies a bound without consulting
    /// any unfinished value bound. The kernel still checks its actual proof.
    fn literal_bound_from_fact(
        &mut self,
        index: usize,
        subject: &ScalarTerm,
        integer: IntegerType,
        direction: Direction,
        current: Option<IntegerValue>,
    ) -> Option<Bound> {
        // Compare the adjusted endpoint before building its proof: x < 10 can
        // tighten an existing x <= 10 even though the written endpoints tie.
        let improves = |literal: &ScalarTerm, strict: bool| {
            let Some((_, value)) = literal.integer_value() else {
                return false;
            };
            let value = if strict {
                step(value, direction == Direction::Lower)
            } else {
                Some(value)
            };
            value.is_some_and(|value| {
                current.is_none_or(|current| direction.prefers(integer.compare(value, current)))
            })
        };
        match (&self.facts[index].proposition, direction) {
            // Direct bounds.
            (Proposition::LessOrEqual(left, literal), Direction::Upper)
            | (Proposition::LessOrEqual(literal, left), Direction::Lower)
                if left == subject && improves(literal, false) =>
            {
                let proof = fact_proof_for_literal_bound(&self.facts, index, self.activity);
                let (literal, proof) = self.literal_endpoint(proof, literal, direction)?;
                Some(Bound { literal, proof })
            }
            (Proposition::LessThan(left, literal), Direction::Upper)
            | (Proposition::LessThan(literal, left), Direction::Lower)
                if left == subject && improves(literal, true) =>
            {
                let proof = fact_proof_for_literal_bound(&self.facts, index, self.activity);
                let (value, proof) = self.literal_endpoint(proof, literal, direction)?;
                let adjacent = step(value, direction == Direction::Lower)?;
                let conclusion = Self::oriented(
                    subject,
                    ScalarTerm::integer(integer, adjacent).ok()?,
                    direction,
                );
                Some(Bound {
                    literal: adjacent,
                    proof: node(
                        conclusion,
                        ProofRule::IntegerOrderDiscreteness {
                            relation: Box::new(proof),
                        },
                    ),
                })
            }
            (Proposition::Equal(left, right), _)
                if (left == subject && improves(right, false))
                    || (right == subject && improves(left, false)) =>
            {
                // Orient the equality so the weakened `<=` points the right way.
                let proof = fact_proof_for_literal_bound(&self.facts, index, self.activity);
                let wanted_left_is_subject = direction == Direction::Upper;
                let proof = if (left == subject) == wanted_left_is_subject {
                    proof
                } else {
                    flip(proof).ok()?
                };
                let literal = if left == subject { right } else { left };
                let (literal, proof) =
                    self.literal_endpoint(weaken(proof).ok()?, literal, direction)?;
                Some(Bound { literal, proof })
            }
            _ => None,
        }
    }

    /// A bound on `subject` from one fact, or `None` when the fact says
    /// nothing in this direction.
    fn bound_from_fact(
        &mut self,
        index: usize,
        subject: &ScalarTerm,
        integer: IntegerType,
        direction: Direction,
        current: Option<IntegerValue>,
    ) -> Option<Bound> {
        let axiom = self.facts[index].axiom;
        let proposition = self.facts[index].proposition.clone();
        match (&proposition, direction) {
            // A public mathematical equality can bound a runtime value
            // without becoming an unchecked machine-operation definition.
            // First prove the expression's bound, then substitute this exact
            // equality. The kernel checks both premises and the carrier form.
            (Proposition::IntegerMathEqual(left, right), _) => {
                if integer.carrier() != IntegerCarrier::Fixed || integer.is_address() {
                    return None;
                }
                let embedded = IntegerMathTerm::embed(subject.clone())?;
                let expression = if left == &embedded {
                    right
                } else if right == &embedded {
                    left
                } else {
                    return None;
                };
                let image = self.arithmetic_image(expression, direction).ok()?;
                Self::carrier_bound_from_image(
                    subject,
                    image,
                    self.facts[index].proof(&self.facts),
                    direction,
                )
            }
            (Proposition::IntegerMathLessOrEqual(left, right), _) => {
                if integer.carrier() != IntegerCarrier::Fixed || integer.is_address() {
                    return None;
                }
                let embedded = IntegerMathTerm::embed(subject.clone())?;
                let chained = match (direction, left == &embedded, right == &embedded) {
                    (Direction::Upper, true, false) => {
                        let image = self.arithmetic_image(right, Direction::Upper).ok()?;
                        let Proposition::IntegerMathLessOrEqual(_, limit) = &image.conclusion
                        else {
                            return None;
                        };
                        node(
                            Proposition::IntegerMathLessOrEqual(embedded.clone(), limit.clone()),
                            ProofRule::IntegerLessOrEqualTransitivity {
                                left_less_or_equal_middle: Box::new(
                                    self.facts[index].proof(&self.facts),
                                ),
                                middle_less_or_equal_right: Box::new(image),
                            },
                        )
                    }
                    (Direction::Lower, false, true) => {
                        let image = self.arithmetic_image(left, Direction::Lower).ok()?;
                        let Proposition::IntegerMathLessOrEqual(limit, _) = &image.conclusion
                        else {
                            return None;
                        };
                        node(
                            Proposition::IntegerMathLessOrEqual(limit.clone(), embedded.clone()),
                            ProofRule::IntegerLessOrEqualTransitivity {
                                left_less_or_equal_middle: Box::new(image),
                                middle_less_or_equal_right: Box::new(
                                    self.facts[index].proof(&self.facts),
                                ),
                            },
                        )
                    }
                    _ => return None,
                };
                let unchanged = primitive(
                    Proposition::IntegerMathEqual(embedded.clone(), embedded),
                    PrimitiveJudgment::ReflexiveEquality,
                );
                Self::carrier_bound_from_image(subject, chained, unchanged, direction)
            }
            // Relations to another value: chain through its bound.
            (Proposition::LessOrEqual(near, far), _) | (Proposition::LessThan(near, far), _)
                if matches!(far, ScalarTerm::Value { .. })
                    && matches!(near, ScalarTerm::Value { .. })
                    && ((direction == Direction::Upper && near == subject)
                        || (direction == Direction::Lower && far == subject)) =>
            {
                // A strict relation chains as it is; `through` restates a
                // strict result as `<=` with the literal stepped inward.
                let other = if near == subject { far } else { near };
                self.through_fact(index, subject, other, integer, direction, current)
            }
            (Proposition::Equal(left, right), _)
                if matches!(left, ScalarTerm::Value { .. })
                    && matches!(right, ScalarTerm::Value { .. })
                    && left != right
                    && (left == subject || right == subject) =>
            {
                let other = if left == subject { right } else { left };
                self.through_fact(index, subject, other, integer, direction, current)
            }
            // Result equations: the output's bound from its operands' bounds.
            (Proposition::Equal(output, expression), _) if output == subject && axiom.is_some() => {
                let axiom = axiom?;
                let defined = self.definition(axiom, subject, integer, expression, direction);
                // An unsigned remainder is also below its divisor.
                let below = (direction == Direction::Upper)
                    .then(|| self.remainder_below_divisor(Some(axiom), subject, expression))
                    .flatten()
                    .and_then(|(divisor, relation)| self.through(relation, &divisor, direction));
                // A remainder by a divisor no literal fixes lies between
                // zero and its dividend.
                let within = self.remainder_within_dividend(axiom, subject, expression, direction);
                // A quotient of a nonnegative dividend by a positive divisor
                // is at most the dividend, and so at most its upper bound.
                let quotient = (direction == Direction::Upper)
                    .then(|| self.quotient_order(axiom, subject, expression, false))
                    .flatten()
                    .and_then(|(dividend, relation)| self.through(relation, &dividend, direction));
                // A remainder by a divisor its own bounds fix to a literal.
                let fixed = self.remainder_by_fixed_divisor(axiom, subject, expression, direction);
                // A remainder of a polynomial multiple of its literal divisor is zero.
                let multiple = matches!(expression, ScalarTerm::ExactIntegerRemainder { .. })
                    .then(|| self.multiple_remainder_bound(subject, Some(axiom), direction))
                    .flatten();
                let interior = (direction == Direction::Upper
                    && matches!(expression, ScalarTerm::ExactIntegerAdd { .. }))
                .then(|| {
                    self.interior_sum_edges(subject, expression)
                        .into_iter()
                        .filter_map(|(end, relation, _)| self.through(relation, &end, direction))
                        .reduce(|best, next| {
                            if direction.prefers(integer.compare(next.literal, best.literal)) {
                                next
                            } else {
                                best
                            }
                        })
                })
                .flatten();
                [defined, below, within, quotient, fixed, multiple, interior]
                    .into_iter()
                    .flatten()
                    .reduce(|best, next| {
                        if direction.prefers(integer.compare(next.literal, best.literal)) {
                            next
                        } else {
                            best
                        }
                    })
            }
            _ => self.compound_bound_from_fact(index, subject, integer, direction, current),
        }
    }

    fn compound_order_fact(&mut self, index: usize) -> ProofNode {
        #[cfg(test)]
        {
            let mut work = crate::checking::known::FactProofWork::default();
            let proof = self.facts[index].proof_with_work(&self.facts, &mut work);
            self.activity.compound_order_fact_nodes = self
                .activity
                .compound_order_fact_nodes
                .saturating_add(work.nodes);
            proof
        }
        #[cfg(not(test))]
        {
            self.facts[index].proof(&self.facts)
        }
    }

    fn compound_order_proof(&mut self, order: CompoundOrder) -> Option<ProofNode> {
        match order {
            CompoundOrder::Relation { fact, .. } => Some(self.compound_order_fact(fact)),
            CompoundOrder::Equality { fact } => weaken(self.compound_order_fact(fact)).ok(),
            CompoundOrder::ReversedEquality { fact } => {
                flip(self.compound_order_fact(fact)).and_then(weaken).ok()
            }
        }
    }

    /// Bounds `subject` from an order or non-axiom equality whose endpoints
    /// include exact additions or subtractions, such as `index + 1 < 4` or
    /// `index + 1 < out.length`.
    fn compound_bound_from_fact(
        &mut self,
        index: usize,
        subject: &ScalarTerm,
        integer: IntegerType,
        direction: Direction,
        current: Option<IntegerValue>,
    ) -> Option<Bound> {
        if integer.carrier() != IntegerCarrier::Fixed || integer.is_address() {
            return None;
        }
        let proposition = self.facts[index].proposition.clone();
        let axiom = self.facts[index].axiom;
        let mut orders = Vec::with_capacity(2);
        match &proposition {
            Proposition::LessOrEqual(left, right) | Proposition::LessThan(left, right)
                if !is_bare_scalar(left) || !is_bare_scalar(right) =>
            {
                let wanted = match direction {
                    Direction::Upper => left,
                    Direction::Lower => right,
                };
                if term_mentions(wanted, subject) {
                    orders.push((
                        left.clone(),
                        right.clone(),
                        CompoundOrder::Relation {
                            fact: index,
                            strict: matches!(proposition, Proposition::LessThan(..)),
                        },
                    ));
                }
            }
            Proposition::Equal(left, right)
                if axiom.is_none()
                    && left != right
                    && (!is_bare_scalar(left) || !is_bare_scalar(right)) =>
            {
                // Orient so `subject` stands on the bounded side (`left` for an
                // upper bound, `right` for a lower bound).
                match direction {
                    Direction::Upper => {
                        if term_mentions(left, subject) {
                            orders.push((
                                left.clone(),
                                right.clone(),
                                CompoundOrder::Equality { fact: index },
                            ));
                        }
                        if term_mentions(right, subject) {
                            orders.push((
                                right.clone(),
                                left.clone(),
                                CompoundOrder::ReversedEquality { fact: index },
                            ));
                        }
                    }
                    Direction::Lower => {
                        if term_mentions(right, subject) {
                            orders.push((
                                left.clone(),
                                right.clone(),
                                CompoundOrder::Equality { fact: index },
                            ));
                        }
                        if term_mentions(left, subject) {
                            orders.push((
                                right.clone(),
                                left.clone(),
                                CompoundOrder::ReversedEquality { fact: index },
                            ));
                        }
                    }
                }
            }
            _ => return None,
        }
        orders
            .into_iter()
            .filter_map(|(left, right, order)| {
                let (open_side, limit_side) = match direction {
                    Direction::Upper => (&left, &right),
                    Direction::Lower => (&right, &left),
                };
                let (m_val, le_proof) = self.compound_limit_as_le(
                    open_side, limit_side, order, subject, integer, direction, current,
                )?;
                if open_side == subject {
                    return Some(Bound {
                        literal: m_val,
                        proof: le_proof,
                    });
                }
                let le_proof =
                    if let Some(delta) = exact_constant_offset(open_side, subject, integer) {
                        match self.translate_offset_bound(
                            le_proof, subject, integer, m_val, delta, direction,
                        ) {
                            OffsetTranslation::Translated(exact) => return Some(exact),
                            OffsetTranslation::Original(proof) => proof,
                        }
                    } else {
                        le_proof
                    };
                self.monotone_open_bound(open_side, le_proof, subject, integer, m_val, direction)
            })
            .reduce(|best, next| {
                if direction.prefers(integer.compare(next.literal, best.literal)) {
                    next
                } else {
                    best
                }
            })
    }

    /// Turns the selected order (`open_side R limit_side` for `Upper`, or
    /// `limit_side R open_side` for `Lower`) into a non-strict literal order
    /// `open_side <= m` or `m <= open_side`.
    fn compound_limit_as_le(
        &mut self,
        open_side: &ScalarTerm,
        limit_side: &ScalarTerm,
        order: CompoundOrder,
        subject: &ScalarTerm,
        integer: IntegerType,
        direction: Direction,
        current: Option<IntegerValue>,
    ) -> Option<(IntegerValue, ProofNode)> {
        // A direct endpoint needs no later offset or monotone projection.
        // Settle the original queries first; only its unused proof is omitted.
        let improves_direct = |literal: IntegerValue, strict: bool| {
            if open_side != subject {
                return true;
            }
            let literal = if strict {
                step(literal, direction == Direction::Lower)
            } else {
                Some(literal)
            };
            literal.is_some_and(|literal| {
                current.is_none_or(|current| direction.prefers(integer.compare(literal, current)))
            })
        };
        let step_strict =
            |value: IntegerValue, proof: ProofNode| -> Option<(IntegerValue, ProofNode)> {
                if !matches!(proof.conclusion, Proposition::LessThan(..)) {
                    return Some((value, proof));
                }
                let adjacent = step(value, direction == Direction::Lower)?;
                let conclusion = Self::oriented(
                    open_side,
                    ScalarTerm::integer(integer, adjacent).ok()?,
                    direction,
                );
                Some((
                    adjacent,
                    node(
                        conclusion,
                        ProofRule::IntegerOrderDiscreteness {
                            relation: Box::new(proof),
                        },
                    ),
                ))
            };
        if limit_side.integer_value().is_some() {
            let (_, raw) = limit_side.integer_value()?;
            if !improves_direct(raw, order.strict()) {
                return None;
            }
            let order_proof = self.compound_order_proof(order)?;
            let (raw, at_plain) = self.literal_endpoint(order_proof, limit_side, direction)?;
            return step_strict(raw, at_plain);
        }
        match direction {
            Direction::Upper => {
                let (upper, step_out) = if matches!(limit_side, ScalarTerm::Value { .. }) {
                    (limit_side.clone(), None)
                } else {
                    let (sup, step_out) = self.upper_subterm(limit_side)?;
                    (sup, Some(step_out))
                };
                let ScalarTerm::Value { id, .. } = upper else {
                    return None;
                };
                if upper == *subject {
                    return None;
                }
                let literal = self.bound_literal(id, Direction::Upper)?;
                let strict = order.strict()
                    || step_out
                        .as_ref()
                        .is_some_and(|proof| matches!(proof.conclusion, Proposition::LessThan(..)));
                if !improves_direct(literal, strict) {
                    return None;
                }
                let upper_bound = self.bound(id, Direction::Upper)?;
                let mut chained = self.compound_order_proof(order)?;
                if let Some(step_out) = step_out {
                    chained = chain(chained, step_out).ok()?;
                }
                chained = chain(chained, upper_bound.proof).ok()?;
                step_strict(upper_bound.literal, chained)
            }
            Direction::Lower => {
                let mut candidates = Vec::new();
                if let ScalarTerm::Value { id, .. } = limit_side {
                    if limit_side != subject
                        && let Some(literal) = self.bound_literal(*id, Direction::Lower)
                        && improves_direct(literal, order.strict())
                        && let Some(lower_bound) = self.bound(*id, Direction::Lower)
                        && let Some(order_proof) = self.compound_order_proof(order)
                        && let Ok(chained) = chain(lower_bound.proof, order_proof)
                        && let Some(candidate) = step_strict(lower_bound.literal, chained)
                    {
                        candidates.push(candidate);
                    }
                } else {
                    let mut order_proof = None;
                    let mut leaves = Vec::new();
                    limit_side.visit_value_ids(|id| {
                        if !leaves.contains(&id) {
                            leaves.push(id);
                        }
                        true
                    });
                    for id in leaves {
                        if self.values.scalar_type(id) != Some(ScalarType::Integer(integer)) {
                            continue;
                        }
                        let leaf = ScalarTerm::value(id, ScalarType::Integer(integer));
                        if &leaf == subject {
                            continue;
                        }
                        let Some(leaf_to_limit) = self.below_compound(&leaf, limit_side) else {
                            continue;
                        };
                        let Some(literal) = self.bound_literal(id, Direction::Lower) else {
                            continue;
                        };
                        let strict = matches!(leaf_to_limit.conclusion, Proposition::LessThan(..))
                            || order.strict();
                        if !improves_direct(literal, strict) {
                            continue;
                        }
                        let Some(lower_bound) = self.bound(id, Direction::Lower) else {
                            continue;
                        };
                        if order_proof.is_none() {
                            order_proof = Some(self.compound_order_proof(order)?);
                        }
                        let proof = order_proof.as_ref()?;
                        if let Ok(first) = chain(lower_bound.proof, leaf_to_limit)
                            && let Ok(chained) = chain(first, proof.clone())
                            && let Some(candidate) = step_strict(lower_bound.literal, chained)
                        {
                            candidates.push(candidate);
                        }
                    }
                }
                candidates.into_iter().reduce(|best, next| {
                    if direction.prefers(integer.compare(next.0, best.0)) {
                        next
                    } else {
                        best
                    }
                })
            }
        }
    }

    /// Translate a checked mathematical order, then independently check the
    /// endpoint equalities that restate it as the requested order.
    fn translate_mathematical_order(
        &self,
        relation: ProofNode,
        addend: IntegerMathTerm,
        goal: &Proposition,
    ) -> Option<ProofNode> {
        let translation =
            self.prepare_mathematical_order_translation(&relation.conclusion, addend, goal)?;
        Some(translation.apply(relation))
    }

    fn prepare_mathematical_order_translation(
        &self,
        relation: &Proposition,
        addend: IntegerMathTerm,
        goal: &Proposition,
    ) -> Option<MathematicalOrderTranslation> {
        let Proposition::IntegerMathLessOrEqual(left, right) = relation else {
            return None;
        };
        let lifted = lift_exact_integer_order(goal);
        let Proposition::IntegerMathLessOrEqual(goal_left, goal_right) =
            lifted.as_ref().unwrap_or(goal)
        else {
            return None;
        };
        let translated_left =
            IntegerMathTerm::Add(Box::new(left.clone()), Box::new(addend.clone()));
        let translated_right =
            IntegerMathTerm::Add(Box::new(right.clone()), Box::new(addend.clone()));
        let translated_conclusion =
            Proposition::IntegerMathLessOrEqual(translated_left.clone(), translated_right.clone());
        let left_equality = self.closed(sorted_math_equal(translated_left, goal_left.clone()))?;
        let left_substituted_conclusion =
            Proposition::IntegerMathLessOrEqual(goal_left.clone(), translated_right.clone());
        let right_equality =
            self.closed(sorted_math_equal(translated_right, goal_right.clone()))?;
        Some(MathematicalOrderTranslation {
            addend,
            translated_conclusion,
            left_substituted_conclusion,
            goal: goal.clone(),
            left_equality,
            right_equality,
        })
    }

    /// Translates `open_side <= m_val` (or `m_val <= open_side`) by `-delta`
    /// where `open_side` denotes `subject + delta`. Failed preparation returns
    /// the original relation for the unchanged monotone fallback.
    fn translate_offset_bound(
        &self,
        le_proof: ProofNode,
        subject: &ScalarTerm,
        integer: IntegerType,
        m_val: IntegerValue,
        delta: i128,
        direction: Direction,
    ) -> OffsetTranslation {
        let Some(shifted) = shift_integer_value(integer, m_val, delta) else {
            return OffsetTranslation::Original(le_proof);
        };
        let Ok(m_term) = ScalarTerm::integer(integer, m_val) else {
            return OffsetTranslation::Original(le_proof);
        };
        let Ok(target_term) = ScalarTerm::integer(integer, shifted) else {
            return OffsetTranslation::Original(le_proof);
        };
        let Proposition::LessOrEqual(le_left, le_right) = &le_proof.conclusion else {
            return OffsetTranslation::Original(le_proof);
        };
        let Some(math_left) = IntegerMathTerm::embed(le_left.clone()) else {
            return OffsetTranslation::Original(le_proof);
        };
        let Some(math_right) = IntegerMathTerm::embed(le_right.clone()) else {
            return OffsetTranslation::Original(le_proof);
        };
        let embedded_le = Proposition::IntegerMathLessOrEqual(math_left, math_right);
        let goal = Self::oriented(subject, target_term, direction);
        let Some(m_refl) = self.reflexive(Proposition::Equal(m_term.clone(), m_term)) else {
            return OffsetTranslation::Original(le_proof);
        };
        let Some(negative_delta) = delta.checked_neg() else {
            return OffsetTranslation::Original(le_proof);
        };
        let addend = IntegerMathTerm::literal(IntegerValue::Signed(negative_delta));
        let Some(translation) =
            self.prepare_mathematical_order_translation(&embedded_le, addend, &goal)
        else {
            return OffsetTranslation::Original(le_proof);
        };
        let endpoint = match direction {
            Direction::Upper => 1,
            Direction::Lower => 0,
        };
        let math_le = node(
            embedded_le,
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(le_proof),
                equality: Box::new(m_refl),
                endpoint,
            },
        );
        OffsetTranslation::Translated(Bound {
            literal: shifted,
            proof: translation.apply(math_le),
        })
    }

    /// Monotone fallback on `open_side <= m_val` (or `m_val <= open_side`) when
    /// `open_side` adds or subtracts an open nonnegative/positive value.
    fn monotone_open_bound(
        &mut self,
        open_side: &ScalarTerm,
        le_proof: ProofNode,
        subject: &ScalarTerm,
        integer: IntegerType,
        m_val: IntegerValue,
        direction: Direction,
    ) -> Option<Bound> {
        let chained = match direction {
            Direction::Upper => chain(self.below_compound(subject, open_side)?, le_proof).ok()?,
            Direction::Lower => {
                if !Self::upper_subterm_can_reach(open_side, |candidate| candidate == subject) {
                    return None;
                }
                let (sup, step_out) = self.upper_subterm(open_side)?;
                if &sup != subject {
                    return None;
                }
                chain(le_proof, step_out).ok()?
            }
        };
        if !matches!(chained.conclusion, Proposition::LessThan(..)) {
            return Some(Bound {
                literal: m_val,
                proof: chained,
            });
        }
        let adjacent = step(m_val, direction == Direction::Lower)?;
        let conclusion = Self::oriented(
            subject,
            ScalarTerm::integer(integer, adjacent).ok()?,
            direction,
        );
        Some(Bound {
            literal: adjacent,
            proof: node(
                conclusion,
                ProofRule::IntegerOrderDiscreteness {
                    relation: Box::new(chained),
                },
            ),
        })
    }

    /// A non-tightening relation leaves the earlier certificate selected.
    /// Settle its other endpoint in the original order, then avoid expanding
    /// the fact and copying that endpoint's proof when neither can be used.
    fn through_fact(
        &mut self,
        index: usize,
        subject: &ScalarTerm,
        other: &ScalarTerm,
        integer: IntegerType,
        direction: Direction,
        current: Option<IntegerValue>,
    ) -> Option<Bound> {
        let ScalarTerm::Value { id, .. } = other else {
            return None;
        };
        let mut literal = self.bound_literal(*id, direction)?;
        if matches!(self.facts[index].proposition, Proposition::LessThan(..)) {
            literal = step(literal, direction == Direction::Lower)?;
        }
        if current.is_some_and(|current| !direction.prefers(integer.compare(literal, current))) {
            return None;
        }
        let proof = self.facts[index].proof(&self.facts);
        let relation = if let Proposition::Equal(left, _) = &proof.conclusion {
            let wanted_left_is_subject = direction == Direction::Upper;
            let proof = if (left == subject) == wanted_left_is_subject {
                proof
            } else {
                flip(proof).ok()?
            };
            weaken(proof).ok()?
        } else {
            proof
        };
        self.through(relation, other, direction)
    }

    /// Bound `subject` through `relation` (`subject R other` for an upper
    /// bound, `other R subject` for a lower one) and `other`'s bound.
    fn through(
        &mut self,
        relation: ProofNode,
        other: &ScalarTerm,
        direction: Direction,
    ) -> Option<Bound> {
        let ScalarTerm::Value { id, .. } = other else {
            return None;
        };
        let bound = self.bound(*id, direction)?;
        let proof = match direction {
            Direction::Upper => chain(relation, bound.proof).ok()?,
            Direction::Lower => chain(bound.proof, relation).ok()?,
        };
        // A strict chain ends in `<`; step its literal inward so the bound is
        // stated as `<=` like every other.
        if matches!(proof.conclusion, Proposition::LessThan(..)) {
            let (_, left, right) = order(&proof.conclusion)?;
            let (left, right) = (left.clone(), right.clone());
            let literal = step(bound.literal, direction == Direction::Lower)?;
            let integer = left.scalar_type().as_integer()?;
            let literal_term = ScalarTerm::integer(integer, literal).ok()?;
            let conclusion = match direction {
                Direction::Upper => Proposition::LessOrEqual(left, literal_term),
                Direction::Lower => Proposition::LessOrEqual(literal_term, right),
            };
            return Some(Bound {
                literal,
                proof: node(
                    conclusion,
                    ProofRule::IntegerOrderDiscreteness {
                        relation: Box::new(proof),
                    },
                ),
            });
        }
        Some(Bound {
            literal: bound.literal,
            proof,
        })
    }

    /// A bound on the output of an exact add, subtract or multiply from its
    /// operands' bounds, citing the result equation as the definition.
    fn definition(
        &mut self,
        axiom: usize,
        subject: &ScalarTerm,
        integer: IntegerType,
        expression: &ScalarTerm,
        direction: Direction,
    ) -> Option<Bound> {
        let (left, right, right_direction, add) = match expression {
            ScalarTerm::ExactIntegerMultiply { .. } => {
                return self.product(axiom, subject, integer, expression, direction);
            }
            ScalarTerm::ExactIntegerAdd { left, right, .. } => (left, right, direction, true),
            ScalarTerm::ExactIntegerSubtract { left, right, .. } => {
                (left, right, direction.opposite(), false)
            }
            ScalarTerm::IntegerWiden { .. } | ScalarTerm::IntegerExactCast { .. } => {
                return self.cast_bound(axiom, subject, integer, expression, direction);
            }
            ScalarTerm::ExactIntegerShiftLeft { value, count, .. } => {
                return self
                    .chain_step(axiom, subject, integer, expression, direction)
                    .or_else(|| self.shifted(axiom, subject, integer, value, count, direction));
            }
            _ => return self.chain_step(axiom, subject, integer, expression, direction),
        };
        let left_proof = self.evidence(left, direction).ok()?;
        let right_proof = self.evidence(right, right_direction).ok()?;
        let witness = IntegerAffineWitness {
            root: left.as_ref().clone(),
            target: expression.clone(),
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        };
        let form = check_integer_affine_witness(self.context, self.axioms, &witness).ok()?;
        let evidence = Proposition::Conjunction(vec![
            left_proof.conclusion.clone(),
            right_proof.conclusion.clone(),
        ]);
        let mapped = map_integer_affine_bound(&form, &evidence).ok()?;
        let literal = match (&mapped, direction) {
            (
                Proposition::IntegerMathLessOrEqual(_, IntegerMathTerm::IntegerLiteral(literal)),
                Direction::Upper,
            )
            | (
                Proposition::IntegerMathLessOrEqual(IntegerMathTerm::IntegerLiteral(literal), _),
                Direction::Lower,
            ) => literal.as_integer_value(integer)?,
            _ => return None,
        };
        let conclusion = Self::oriented(
            subject,
            ScalarTerm::integer(integer, literal).ok()?,
            direction,
        );
        let rule = if add {
            ProofRule::IntegerExactAddDefinitionBound {
                left_bound: Box::new(left_proof),
                right_bound: Box::new(right_proof),
                definition_axiom: axiom,
            }
        } else {
            ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound: Box::new(left_proof),
                right_bound: Box::new(right_proof),
                definition_axiom: axiom,
            }
        };
        Some(Bound {
            literal,
            proof: node(conclusion, rule),
        })
    }

    /// A bound on the output of an exact multiply. The kernel has no
    /// definition rule for a product and needs none: it bounds the
    /// mathematical product term (`image`), carries that bound through the
    /// result equation onto the output value, and restates it in the
    /// carrier. A corner product outside the carrier gives no bound here,
    /// which leaves the output its carrier bound.
    fn product(
        &mut self,
        axiom: usize,
        subject: &ScalarTerm,
        integer: IntegerType,
        expression: &ScalarTerm,
        direction: Direction,
    ) -> Option<Bound> {
        let ScalarTerm::Value { id, .. } = subject else {
            return None;
        };
        let ScalarTerm::ExactIntegerMultiply { left, right, .. } = expression else {
            return None;
        };
        let image = self
            .image(
                Arithmetic::Multiply,
                left,
                right,
                expression.clone(),
                direction,
            )
            .ok()?;
        self.transported(axiom, *id, integer, image, direction)
    }

    /// A bound on the output of an exact left shift whose count is a value
    /// no earlier result equation fixes to a literal, so `chain_step` has
    /// no landing for it. The count is then known only from a hypothesis
    /// (an entry `requires`, a guard, a callee `ensures`) or its own bound:
    /// the kernel bounds the mathematical shift from the value's bound and
    /// that knowledge (`shift_left_image`), and the bound is carried onto
    /// the output through the result equation, as for a product.
    fn shifted(
        &mut self,
        axiom: usize,
        subject: &ScalarTerm,
        integer: IntegerType,
        value: &ScalarTerm,
        count: &ScalarTerm,
        direction: Direction,
    ) -> Option<Bound> {
        let ScalarTerm::Value { id, .. } = subject else {
            return None;
        };
        let leaf = |term: &ScalarTerm| match term {
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(source_type),
            } => Some(IntegerMathTerm::MathValue {
                source_type: *source_type,
                value: *id,
            }),
            _ => None,
        };
        let (value, count) = (leaf(value)?, leaf(count)?);
        let image = self.shift_left_image(&value, &count, direction).ok()?;
        self.transported(axiom, *id, integer, image, direction)
    }

    /// Restate a checked mathematical image at its exact carrier endpoint.
    fn carrier_bound_from_image(
        subject: &ScalarTerm,
        image: ProofNode,
        equality: ProofNode,
        direction: Direction,
    ) -> Option<Bound> {
        let (limit, endpoint) = match (&image.conclusion, direction) {
            (Proposition::IntegerMathLessOrEqual(_, limit), Direction::Upper) => (limit, 0),
            (Proposition::IntegerMathLessOrEqual(limit, _), Direction::Lower) => (limit, 1),
            _ => return None,
        };
        let IntegerMathTerm::IntegerLiteral(limit) = limit else {
            return None;
        };
        let integer = subject.scalar_type().as_integer()?;
        let literal = limit.as_integer_value(integer)?;
        let goal = Self::oriented(
            subject,
            ScalarTerm::integer(integer, literal).ok()?,
            direction,
        );
        Some(Bound {
            literal,
            proof: node(
                goal,
                ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(image),
                    equality: Box::new(equality),
                    endpoint,
                },
            ),
        })
    }

    /// The output `output` of the result equation `axiom` bounded by
    /// `image`, the kernel's bound on the equation's mathematical term:
    /// the bound carried through the equation (`ValueEqualityTransport`)
    /// and restated in the output's carrier (`IntegerOrderSubstitution`).
    /// A limit outside the carrier gives no bound.
    fn transported(
        &self,
        axiom: usize,
        output: ValueId,
        integer: IntegerType,
        image: ProofNode,
        direction: Direction,
    ) -> Option<Bound> {
        let subject = &ScalarTerm::value(output, ScalarType::Integer(integer));
        let limit = match (&image.conclusion, direction) {
            (Proposition::IntegerMathLessOrEqual(_, limit), Direction::Upper)
            | (Proposition::IntegerMathLessOrEqual(limit, _), Direction::Lower) => limit.clone(),
            _ => return None,
        };
        let output = IntegerMathTerm::MathValue {
            source_type: integer,
            value: output,
        };
        let on_output = match direction {
            Direction::Upper => Proposition::IntegerMathLessOrEqual(output.clone(), limit),
            Direction::Lower => Proposition::IntegerMathLessOrEqual(limit, output.clone()),
        };
        let equation = node(
            self.axioms.get(axiom)?.clone(),
            ProofRule::SemanticAxiom { index: axiom },
        );
        let transported = node(
            on_output,
            ProofRule::ValueEqualityTransport {
                premise: Box::new(image),
                equalities: vec![equation],
            },
        );
        // Substituting the output for itself changes nothing mathematically.
        // The rule is used because it lets a mathematical order conclude in
        // its carrier spelling, the form every other bound has.
        let unchanged = primitive(
            Proposition::IntegerMathEqual(output.clone(), output),
            PrimitiveJudgment::ReflexiveEquality,
        );
        Self::carrier_bound_from_image(subject, transported, unchanged, direction)
    }

    /// The kernel's image of the operands' bounds through `target`, an exact
    /// operation on `first` and `second`: a bound in `direction` on the
    /// operation's mathematical term.
    fn image(
        &mut self,
        operation: Arithmetic,
        first: &ScalarTerm,
        second: &ScalarTerm,
        target: ScalarTerm,
        direction: Direction,
    ) -> Result<ProofNode, Unproven> {
        let opposite = direction.opposite();
        let parts = match operation {
            Arithmetic::Add => vec![
                self.evidence(first, direction)?,
                self.evidence(second, direction)?,
            ],
            Arithmetic::Subtract => vec![
                self.evidence(first, direction)?,
                self.evidence(second, opposite)?,
            ],
            // Multiply takes both bounds of each operand, the goal's own
            // direction first (`map_direct_multiply_bound`).
            Arithmetic::Multiply => vec![
                self.evidence(first, direction)?,
                self.evidence(first, opposite)?,
                self.evidence(second, direction)?,
                self.evidence(second, opposite)?,
            ],
        };
        let root =
            Proposition::Conjunction(parts.iter().map(|part| part.conclusion.clone()).collect());
        let witness = IntegerAffineWitness {
            root: first.clone(),
            target,
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        };
        let form = check_integer_affine_witness(self.context, self.axioms, &witness)
            .map_err(|_| "the kernel refuses the operation's witness")?;
        let image = map_integer_affine_bound(&form, &root)
            .map_err(|_| "the kernel maps no bound through the operation")?;
        Ok(node(
            image,
            ProofRule::IntegerAffineBound {
                root_bound: Box::new(node(root, ProofRule::ConjunctionIntroduction(parts))),
                witness,
            },
        ))
    }

    /// `left <= right` over mathematical integers, where one side is an
    /// exact add, subtract or multiply of two operands and the other a
    /// literal: the representability goal of an exact operation.
    fn arithmetic(
        &mut self,
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Result<ProofNode, Unproven> {
        let interval = self.arithmetic_interval(left, right);
        if interval.is_ok() {
            return interval;
        }
        self.correlated_subtract(left, right)
            .or_else(|| self.mathematical_offset_order(left, right))
            .ok_or(interval.err().unwrap_or("no bound"))
    }

    /// `0 <= a - b` for an unsigned exact subtract, from `b <= a` between the
    /// two values: the kernel's correlated unsigned subtract maps that
    /// relation to the lower carrier bound of the difference.
    fn correlated_subtract(
        &mut self,
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Option<ProofNode> {
        let (IntegerMathTerm::IntegerLiteral(limit), IntegerMathTerm::Subtract(first, second)) =
            (left, right)
        else {
            return None;
        };
        let (
            IntegerMathTerm::MathValue {
                source_type,
                value: minuend,
            },
            IntegerMathTerm::MathValue {
                source_type: other,
                value: subtrahend,
            },
        ) = (first.as_ref(), second.as_ref())
        else {
            return None;
        };
        if source_type != other || limit.magnitude() != 0 || limit.negative() {
            return None;
        }
        let scalar_type = ScalarType::Integer(*source_type);
        let minuend = ScalarTerm::value(*minuend, scalar_type);
        let subtrahend = ScalarTerm::value(*subtrahend, scalar_type);
        let relation = self
            .prove(&Proposition::LessOrEqual(
                subtrahend.clone(),
                minuend.clone(),
            ))
            .ok()?;
        let witness = IntegerAffineWitness {
            root: subtrahend.clone(),
            target: ScalarTerm::exact_integer_subtract(*source_type, minuend, subtrahend).ok()?,
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        };
        let form = check_integer_affine_witness(self.context, self.axioms, &witness).ok()?;
        let image = map_integer_affine_bound(&form, &relation.conclusion).ok()?;
        let goal = Proposition::IntegerMathLessOrEqual(left.clone(), right.clone());
        (image == goal).then(|| {
            node(
                goal,
                ProofRule::IntegerAffineBound {
                    root_bound: Box::new(relation),
                    witness,
                },
            )
        })
    }

    fn arithmetic_interval(
        &mut self,
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Result<ProofNode, Unproven> {
        let (term, limit, direction) = match (left, right) {
            (term, IntegerMathTerm::IntegerLiteral(_)) => (term, right, Direction::Upper),
            (IntegerMathTerm::IntegerLiteral(_), term) => (term, left, Direction::Lower),
            (term @ IntegerMathTerm::Add(..), _) => (term, right, Direction::Upper),
            (_, term @ IntegerMathTerm::Add(..)) => (term, left, Direction::Lower),
            _ => return Err("arithmetic goal between two open terms"),
        };
        let (interval, related) = self.arithmetic_images(term, direction, Some(limit))?;
        let interval = interval.and_then(|image| self.land(image, term, limit, direction));
        if interval.is_ok() {
            return interval;
        }
        if let Some(related) = related
            && let Ok(proof) = self.land(related, term, limit, direction)
        {
            return Ok(proof);
        }
        interval
    }

    /// Choose the tightest comparable endpoint for callers that need a
    /// literal bound. Safety goals can try the correlated proof separately.
    fn arithmetic_image(
        &mut self,
        term: &IntegerMathTerm,
        direction: Direction,
    ) -> Result<ProofNode, Unproven> {
        let (interval, related) = self.arithmetic_images(term, direction, None)?;
        let Some(related) = related else {
            return interval;
        };
        let Ok(interval) = interval else {
            return Ok(related);
        };
        let limit = |proof: &ProofNode| match (&proof.conclusion, direction) {
            (Proposition::IntegerMathLessOrEqual(_, limit), Direction::Upper)
            | (Proposition::IntegerMathLessOrEqual(limit, _), Direction::Lower) => {
                Some(limit.clone())
            }
            _ => None,
        };
        let stronger = limit(&related)
            .zip(limit(&interval))
            .is_some_and(|(related, interval)| {
                direction.prefers(
                    compare_integer_math_terms(&related, &interval)
                        .ok()
                        .flatten(),
                )
            });
        Ok(if stronger { related } else { interval })
    }

    /// Bound the mathematical expression itself. A correlated premise can
    /// retain an open endpoint the operands' separate ranges lose.
    fn arithmetic_images(
        &mut self,
        term: &IntegerMathTerm,
        direction: Direction,
        preferred: Option<&IntegerMathTerm>,
    ) -> Result<(Result<ProofNode, Unproven>, Option<ProofNode>), Unproven> {
        if let IntegerMathTerm::Embed(scalar) = term {
            let Some(Proposition::IntegerMathLessOrEqual(expanded, _)) = lift_exact_integer_order(
                &Proposition::LessOrEqual(scalar.as_ref().clone(), scalar.as_ref().clone()),
            ) else {
                return Err("embedded arithmetic is not exact integer arithmetic");
            };
            if &expanded == term {
                return Err("embedded arithmetic does not expand");
            }
            let (first, second) = if term <= &expanded {
                (term, &expanded)
            } else {
                (&expanded, term)
            };
            let equality = self
                .closed(Proposition::IntegerMathEqual(first.clone(), second.clone()))
                .ok_or("the kernel refuses the exact arithmetic identity")?;
            let (interval, related) = self.arithmetic_images(&expanded, direction, preferred)?;
            let restate = |image: ProofNode| -> Result<ProofNode, Unproven> {
                let (conclusion, endpoint) = match (&image.conclusion, direction) {
                    (Proposition::IntegerMathLessOrEqual(actual, limit), Direction::Upper)
                        if actual == &expanded =>
                    {
                        (
                            Proposition::IntegerMathLessOrEqual(term.clone(), limit.clone()),
                            0,
                        )
                    }
                    (Proposition::IntegerMathLessOrEqual(limit, actual), Direction::Lower)
                        if actual == &expanded =>
                    {
                        (
                            Proposition::IntegerMathLessOrEqual(limit.clone(), term.clone()),
                            1,
                        )
                    }
                    _ => return Err("the expanded arithmetic bound names a different term"),
                };
                Ok(node(
                    conclusion,
                    ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(image),
                        equality: Box::new(equality.clone()),
                        endpoint,
                    },
                ))
            };
            return Ok((
                interval.and_then(restate),
                related.and_then(|image| restate(image).ok()),
            ));
        }
        if let IntegerMathTerm::Add(inner, addend) = term
            && matches!(addend.as_ref(), IntegerMathTerm::IntegerLiteral(_))
            && !matches!(
                inner.as_ref(),
                IntegerMathTerm::MathValue { .. } | IntegerMathTerm::IntegerLiteral(_)
            )
            // A literal-only inner term has no value to translate; the kernel
            // evaluates it, and recursing here only repeats a failing search.
            && !closed_mathematical(inner, &|_| false)
        {
            let inner_bound = self.arithmetic_image(inner, direction)?;
            let Proposition::IntegerMathLessOrEqual(left, right) = &inner_bound.conclusion else {
                return Err("the inner arithmetic bound is not an order");
            };
            let conclusion = Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Add(Box::new(left.clone()), addend.clone()),
                IntegerMathTerm::Add(Box::new(right.clone()), addend.clone()),
            );
            return Ok((
                Ok(node(
                    conclusion,
                    ProofRule::IntegerMathOrderTranslation {
                        relation: Box::new(inner_bound),
                        addend: (**addend).clone(),
                    },
                )),
                None,
            ));
        }
        if let IntegerMathTerm::Add(first, second) = term {
            let target = arithmetic_target(Arithmetic::Add, first, second).ok();
            let interval = target
                .as_ref()
                .ok_or("sum has no scalar signature")
                .and_then(|target| {
                    let operands = target.operands();
                    let [left, right] = operands.as_slice() else {
                        return Err("sum term does not have two operands");
                    };
                    self.image(Arithmetic::Add, left, right, target.clone(), direction)
                })
                .or_else(|_| self.mathematical_sum_image(first, second, direction));
            if interval.as_ref().is_ok_and(|image| {
                preferred.is_some_and(|limit| {
                    mathematical_bounds::image_satisfies(image, limit, direction)
                })
            }) {
                return Ok((interval, None));
            }
            let related = self.related_image(target.as_ref(), term, direction, preferred);
            return Ok((interval, related));
        }
        let (operation, first, second) = match term {
            IntegerMathTerm::Add(first, second) => (Arithmetic::Add, first, second),
            IntegerMathTerm::Subtract(first, second) => (Arithmetic::Subtract, first, second),
            IntegerMathTerm::Multiply(first, second) => (Arithmetic::Multiply, first, second),
            IntegerMathTerm::ShiftLeft { value, count } => {
                return Ok((self.shift_left_image(value, count, direction), None));
            }
            IntegerMathTerm::MathValue { source_type, value } => {
                return Ok((
                    self.value_image(*source_type, *value, term, direction),
                    None,
                ));
            }
            IntegerMathTerm::IntegerLiteral(_) => {
                let bound = self
                    .closed(Proposition::IntegerMathLessOrEqual(
                        term.clone(),
                        term.clone(),
                    ))
                    .ok_or("the kernel refuses the literal bound");
                return Ok((bound, None));
            }
            _ => return Err("arithmetic goal over a single term"),
        };
        let target = arithmetic_target(operation, first, second)?;
        let operands = target.operands();
        let [first, second] = operands.as_slice() else {
            return Err("arithmetic term does not have two operands");
        };
        let interval = self.image(operation, first, second, target.clone(), direction);
        if interval.as_ref().is_ok_and(|image| {
            preferred
                .is_some_and(|limit| mathematical_bounds::image_satisfies(image, limit, direction))
        }) {
            return Ok((interval, None));
        }
        let related = self.related_image(Some(&target), term, direction, preferred);
        Ok((interval, related))
    }

    /// Separate operand ranges lose constraints such as an addition's remaining
    /// capacity. Offer the retained relations and their exact definitions to the
    /// existing affine checker; it alone decides which bound follows.
    fn related_image(
        &mut self,
        target: Option<&ScalarTerm>,
        term: &IntegerMathTerm,
        direction: Direction,
        preferred: Option<&IntegerMathTerm>,
    ) -> Option<ProofNode> {
        let Some(target) = target else {
            let direct = self.mathematical_relation_image(term, direction, preferred);
            if direct.as_ref().is_some_and(|image| {
                preferred.is_none_or(|limit| {
                    mathematical_bounds::image_satisfies(image, limit, direction)
                })
            }) {
                return direct;
            }
            return self
                .mathematical_composed_image(term, direction, preferred)
                .or(direct);
        };
        let exchanged = match target {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } => ScalarTerm::exact_integer_add(
                *scalar_type,
                right.as_ref().clone(),
                left.as_ref().clone(),
            )
            .ok(),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } => ScalarTerm::exact_integer_multiply(
                *scalar_type,
                right.as_ref().clone(),
                left.as_ref().clone(),
            )
            .ok(),
            _ => None,
        };
        let mut fallback = None;
        let mut transport = None;
        for target in std::iter::once(target).chain(exchanged.as_ref()) {
            for fact in &self.facts {
                let (left, right, strict) = match &fact.proposition {
                    Proposition::LessOrEqual(left, right) => (left, right, false),
                    Proposition::LessThan(left, right) => (left, right, true),
                    _ => continue,
                };
                let order = Proposition::LessOrEqual(left.clone(), right.clone());
                for (root_endpoint, root) in [left, right].into_iter().enumerate() {
                    let mut witness = IntegerAffineWitness {
                        root: root.clone(),
                        target: target.clone(),
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    };
                    if let ScalarTerm::Value { id, .. } = root
                        && let Some(Some(definition)) = self.definitions.get(*id)
                        && let Some(index) = self.facts[self.equations[*definition]].axiom
                    {
                        witness.definition_axioms.push(index);
                        witness.literal_axioms.push(None);
                    }
                    let Ok(form) =
                        check_integer_affine_witness(self.context, self.axioms, &witness)
                    else {
                        continue;
                    };
                    let (conclusion, transported) = match map_integer_affine_bound(&form, &order) {
                        Ok(conclusion) => (conclusion, None),
                        Err(_) => {
                            let ScalarTerm::ExactIntegerAdd { left: operand, .. } = target else {
                                continue;
                            };
                            let aligned = if root_endpoint == 0 {
                                Proposition::LessOrEqual(root.clone(), operand.as_ref().clone())
                            } else {
                                Proposition::LessOrEqual(operand.as_ref().clone(), root.clone())
                            };
                            let Ok(conclusion) = map_integer_affine_bound(&form, &aligned) else {
                                continue;
                            };
                            let scratch = transport.get_or_insert_with(|| {
                                crate::checking::known::TransportInputs::new(self.equations.len())
                            });
                            let Some(equalities) = self.order_transport(&order, &aligned, scratch)
                            else {
                                continue;
                            };
                            (conclusion, Some((aligned, equalities)))
                        }
                    };
                    let (actual, limit, endpoint) = match (&conclusion, direction) {
                        (Proposition::IntegerMathLessOrEqual(actual, limit), Direction::Upper) => {
                            (actual, limit, 0)
                        }
                        (Proposition::IntegerMathLessOrEqual(limit, actual), Direction::Lower) => {
                            (actual, limit, 1)
                        }
                        _ => continue,
                    };
                    let goal = match direction {
                        Direction::Upper => {
                            Proposition::IntegerMathLessOrEqual(term.clone(), limit.clone())
                        }
                        Direction::Lower => {
                            Proposition::IntegerMathLessOrEqual(limit.clone(), term.clone())
                        }
                    };
                    // Commuting the operands must itself have checked evidence.
                    // This also rejects an image for a different expression.
                    let (first, second) = if actual <= term {
                        (actual, term)
                    } else {
                        (term, actual)
                    };
                    let Some(equality) =
                        self.closed(Proposition::IntegerMathEqual(first.clone(), second.clone()))
                    else {
                        continue;
                    };
                    let mut premise = fact.proof(&self.facts);
                    if strict {
                        premise = node(
                            order.clone(),
                            ProofRule::IntegerOrderWeakening {
                                relation: Box::new(premise),
                            },
                        );
                    }
                    if let Some((aligned, equalities)) = transported {
                        premise = node(
                            aligned,
                            ProofRule::ValueEqualityTransport {
                                premise: Box::new(premise),
                                equalities,
                            },
                        );
                    }
                    let preferred = preferred.map_or_else(
                        || matches!(limit, IntegerMathTerm::IntegerLiteral(_)),
                        |preferred| {
                            if preferred == limit {
                                return true;
                            }
                            compare_integer_math_terms(limit, preferred)
                                .ok()
                                .flatten()
                                .is_some_and(|ordering| match direction {
                                    Direction::Upper => ordering.is_le(),
                                    Direction::Lower => ordering.is_ge(),
                                })
                        },
                    );
                    let image = node(
                        conclusion,
                        ProofRule::IntegerAffineBound {
                            root_bound: Box::new(premise),
                            witness,
                        },
                    );
                    let candidate = node(
                        goal,
                        ProofRule::IntegerOrderSubstitution {
                            relation: Box::new(image),
                            equality: Box::new(equality),
                            endpoint,
                        },
                    );
                    if preferred {
                        return Some(candidate);
                    }
                    if fallback.is_none() {
                        fallback = Some(candidate);
                    }
                }
            }
        }
        // Existing scalar successes return above. A literal-bound query has
        // no unmet requested limit that needs another search through the facts.
        if preferred.is_none() {
            return fallback;
        }
        if direction == Direction::Upper
            && let Some(image) = self.remaining_capacity_image(target, term)
            && preferred
                .is_some_and(|limit| mathematical_bounds::image_satisfies(&image, limit, direction))
        {
            return Some(image);
        }
        let mathematical = self.mathematical_relation_image(term, direction, preferred);
        if mathematical.as_ref().is_some_and(|image| {
            preferred
                .is_some_and(|limit| mathematical_bounds::image_satisfies(image, limit, direction))
        }) {
            return mathematical;
        }
        self.mathematical_composed_image(term, direction, preferred)
            .or(fallback)
            .or(mathematical)
    }

    /// A normalized guard may spell the constant or conversion where the
    /// operation names its SSA result. Retain the exact definitions that
    /// make the two orders denote the same operands.
    fn order_transport(
        &self,
        retained: &Proposition,
        aligned: &Proposition,
        transport: &mut crate::checking::known::TransportInputs,
    ) -> Option<Vec<ProofNode>> {
        crate::checking::known::Known::transport_inputs(
            &self.facts,
            &self.equations,
            aligned,
            &self.definitions,
            transport,
        );
        let definitions = || {
            transport
                .selected
                .iter()
                .map(|&index| &self.facts[self.equations[index]].proposition)
        };
        let actual = check_value_equality_denotation(self.context, retained, definitions()).ok()?;
        let needed = check_value_equality_denotation(self.context, aligned, definitions()).ok()?;
        (actual == needed).then(|| {
            transport
                .selected
                .iter()
                .map(|&index| self.facts[self.equations[index]].proof(&self.facts))
                .collect()
        })
    }

    /// The goal `term <= limit` (or `limit <= term`) from `affine`, the
    /// kernel's image bound on `term`: the image itself when it is the goal,
    /// or chained with the closed relation between the image and the limit.
    fn land(
        &mut self,
        affine: ProofNode,
        term: &IntegerMathTerm,
        limit: &IntegerMathTerm,
        direction: Direction,
    ) -> Result<ProofNode, Unproven> {
        let image = affine.conclusion.clone();
        let image_term = match (&image, direction) {
            (Proposition::IntegerMathLessOrEqual(term, _), Direction::Upper)
            | (Proposition::IntegerMathLessOrEqual(_, term), Direction::Lower) => term,
            _ => return Err("the bound image points the other way"),
        };
        if image_term != term {
            return Err("the bound image is not the goal's term");
        }
        let goal = match direction {
            Direction::Upper => Proposition::IntegerMathLessOrEqual(term.clone(), limit.clone()),
            Direction::Lower => Proposition::IntegerMathLessOrEqual(limit.clone(), term.clone()),
        };
        if image == goal {
            return Ok(affine);
        }
        let (image_left, image_right) = match &image {
            Proposition::IntegerMathLessOrEqual(left, right) => (left, right),
            _ => return Err("the bound image is not an order"),
        };
        let (first_step, second_step) = match direction {
            Direction::Upper => (
                affine,
                self.prove_direct_subgoal(&Proposition::IntegerMathLessOrEqual(
                    image_right.clone(),
                    limit.clone(),
                ))
                .map_err(|_| "the operands' bounds exceed the carrier")?,
            ),
            Direction::Lower => (
                self.prove_direct_subgoal(&Proposition::IntegerMathLessOrEqual(
                    limit.clone(),
                    image_left.clone(),
                ))
                .map_err(|_| "the operands' bounds exceed the carrier")?,
                affine,
            ),
        };
        Ok(node(
            goal,
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(first_step),
                middle_less_or_equal_right: Box::new(second_step),
            },
        ))
    }
}

/// Reconstruct an executable arithmetic term only when its operands share the
/// existing scalar signature. Unbounded sums use mathematical bounds directly.
fn arithmetic_target(
    operation: Arithmetic,
    first: &IntegerMathTerm,
    second: &IntegerMathTerm,
) -> Result<ScalarTerm, Unproven> {
    let integer = [first, second]
        .into_iter()
        .find_map(|leaf| match leaf {
            IntegerMathTerm::MathValue { source_type, .. } => Some(*source_type),
            _ => None,
        })
        .ok_or("arithmetic goal with no value operand")?;
    let leaf = |leaf: &IntegerMathTerm| -> Result<ScalarTerm, Unproven> {
        match leaf {
            IntegerMathTerm::MathValue { source_type, value } if *source_type == integer => {
                Ok(ScalarTerm::value(*value, ScalarType::Integer(integer)))
            }
            IntegerMathTerm::IntegerLiteral(literal) => literal
                .as_integer_value(integer)
                .and_then(|value| ScalarTerm::integer(integer, value).ok())
                .ok_or("literal operand outside the carrier"),
            _ => Err("operand is not a value or literal of the operation's type"),
        }
    };
    let (first, second) = (leaf(first)?, leaf(second)?);
    match operation {
        Arithmetic::Add => ScalarTerm::exact_integer_add(integer, first.clone(), second.clone()),
        Arithmetic::Subtract => {
            ScalarTerm::exact_integer_subtract(integer, first.clone(), second.clone())
        }
        Arithmetic::Multiply => {
            ScalarTerm::exact_integer_multiply(integer, first.clone(), second.clone())
        }
    }
    .map_err(|_| "operation term")
}

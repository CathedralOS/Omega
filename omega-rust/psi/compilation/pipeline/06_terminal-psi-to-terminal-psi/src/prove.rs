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

use proof_admission::{
    IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule, check_integer_affine_witness,
    check_value_equality_denotation, compare_integer_math_terms, decide_primitive,
    lift_exact_integer_order, map_integer_affine_bound,
};
use semantic_vocabulary::{
    IdTable, IntegerCarrier, IntegerMathTerm, IntegerType, IntegerValue, Proposition,
    PropositionContext, ScalarTerm, ScalarType, ValueId,
};

#[path = "prove/float_meaning.rs"]
mod float_meaning;
#[path = "prove/ieee_float_order.rs"]
mod ieee_float_order;
#[path = "prove/ranges.rs"]
mod ranges;
#[path = "prove/reach.rs"]
mod reach;
#[cfg(test)]
#[path = "prove/tests.rs"]
mod tests;
#[path = "prove/widths.rs"]
mod widths;

use crate::known::{Fact, ProofValues, is_branch_fact};

/// Why no certificate was built. Diagnostic only.
pub(crate) type Unproven = &'static str;

/// One obligation's case work, shared by all of its recursive searches.
/// Running out supplies no premise or partial certificate.
const CASE_BRANCH_BUDGET: usize = 64;

/// Whether this prover has rules for `goal`'s form at all, decided from the
/// form alone. It mirrors `Prover::prove_direct`: truth and falsehood,
/// conjunctions and disjunctions of covered parts, an implication whose
/// conclusion is covered, integer orders and equalities over terms the rules
/// bound (`reach::within_rules`), a comparison of two
/// closed mathematical terms, and an arithmetic bound of one add, subtract,
/// multiply, left shift or single value against a literal. Anything else the
/// prover can only take from an equal hypothesis, so a failure on it says
/// nothing about the program: it is `UnprovedCause::NoRuleForGoalForm`,
/// published `Incomplete`, not `NoDerivation`.
///
/// `fixed` says which values a result equation fixes to a literal. A
/// mathematical term over such values and literals is closed once those
/// equations are substituted, which is how the kernel reads it
/// (`Prover::by_transport`), so a comparison of two such terms is evaluated,
/// not searched for (`Prover::has_rules_for`).
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
        Proposition::ScalarIeeeFloatComparison { .. } | Proposition::FloatMeaningEqual(_) => true,
        Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
            parts.iter().all(|part| covered(part, fixed, width))
        }
        // The premise is only assumed; the conclusion is what is proved.
        Proposition::Implication { conclusion, .. } => covered(conclusion, fixed, width),
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
            // of one carrier and literals, the shape an exact operation's
            // representability goal has. A deeper term, or one over two
            // carriers, is a clause's (`embed(a) + embed(b) * 10 <= 99`),
            // and nothing here bounds it.
            let leaf = |term: &IntegerMathTerm| match term {
                IntegerMathTerm::MathValue { source_type, .. } => Some(Some(*source_type)),
                IntegerMathTerm::IntegerLiteral(_) => Some(None),
                _ => None,
            };
            let bounded = |term: &IntegerMathTerm| match term {
                IntegerMathTerm::Add(first, second)
                | IntegerMathTerm::Subtract(first, second)
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
                _ => false,
            }
        }
        _ => false,
    }
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

#[derive(Debug, Clone)]
enum Slot {
    Unknown,
    InProgress,
    Done(Option<Bound>),
}

#[cfg(test)]
thread_local! {
    /// How many provers were started on this thread: the reuse tests assert
    /// that re-verifying a saved proof starts none.
    pub(crate) static SEARCHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) struct Prover<'a> {
    context: &'a PropositionContext,
    assumptions: &'a [Proposition],
    axioms: &'a [Proposition],
    values: &'a dyn ProofValues,
    facts: Vec<Fact>,
    /// Checked definitions for transport: Boolean values, literals, exact
    /// initial-content aliases, and semantic integer expressions retaining
    /// their selected arithmetic meaning, each citing its prepared fact.
    equations: Vec<usize>,
    definitions: IdTable<ValueId, Option<usize>>,
    /// `equations` extended by arrivals' compound equalities, which only
    /// `by_transport` cites after `equations` alone fails (`Known`).
    arrival_equations: Vec<usize>,
    arrival_definitions: IdTable<ValueId, Option<usize>>,
    transport: crate::known::TransportInputs,
    /// `[upper, lower]` per value, indexed by `ValueId - 1`.
    bounds: Vec<[Slot; 2]>,
    /// Whether a failed goal may split over a disjunctive fact. Guard
    /// contradiction search disables this to avoid re-entering itself.
    case_split: bool,
    /// Shared with each branch/implication and returned after its search.
    remaining_case_branches: usize,
    /// A branch already assumes one alternative of each cut on this path.
    /// Splitting that exact predicate again supplies no new information.
    active_cuts: Vec<Proposition>,
    /// An auxiliary sign query can revisit its parent through contradiction
    /// search. The same goal under these unchanged premises supplies no proof.
    active_goals: Vec<Proposition>,
}

fn node(conclusion: Proposition, rule: ProofRule) -> ProofNode {
    ProofNode { conclusion, rule }
}

fn primitive(conclusion: Proposition, judgment: PrimitiveJudgment) -> ProofNode {
    node(conclusion, ProofRule::Primitive(judgment))
}

fn value_index(value: ValueId) -> usize {
    usize::try_from(value.get() - 1).unwrap_or(usize::MAX)
}

fn step(value: IntegerValue, increase: bool) -> Option<IntegerValue> {
    match (value, increase) {
        (IntegerValue::Signed(value), true) => value.checked_add(1).map(IntegerValue::Signed),
        (IntegerValue::Signed(value), false) => value.checked_sub(1).map(IntegerValue::Signed),
        (IntegerValue::Unsigned(value), true) => value.checked_add(1).map(IntegerValue::Unsigned),
        (IntegerValue::Unsigned(value), false) => value.checked_sub(1).map(IntegerValue::Unsigned),
    }
}

/// The two sides of a `<=` or `<`, and whether it is strict.
fn order(proposition: &Proposition) -> Option<(bool, &ScalarTerm, &ScalarTerm)> {
    match proposition {
        Proposition::LessOrEqual(left, right) => Some((false, left, right)),
        Proposition::LessThan(left, right) => Some((true, left, right)),
        _ => None,
    }
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

/// `a == b` to `a <= b`.
fn weaken(equality: ProofNode) -> Result<ProofNode, Unproven> {
    let Proposition::Equal(left, right) = &equality.conclusion else {
        return Err("weakening of a non-equality");
    };
    let conclusion = Proposition::LessOrEqual(left.clone(), right.clone());
    Ok(node(
        conclusion,
        ProofRule::IntegerOrderWeakening {
            relation: Box::new(equality),
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
            conjunction: Box::new(both),
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
    ) -> Self {
        #[cfg(test)]
        SEARCHES.with(|searches| searches.set(searches.get() + 1));
        let crate::known::Known {
            facts,
            equations,
            definitions,
            arrival_equations,
            arrival_definitions,
        } = crate::known::Known::new(context, assumptions, axioms, values);
        let transport = crate::known::TransportInputs::new(equations.len());
        Self {
            context,
            assumptions,
            axioms,
            values,
            facts,
            equations,
            definitions,
            arrival_equations,
            arrival_definitions,
            transport,
            bounds: (0..values.count())
                .map(|_| [Slot::Unknown, Slot::Unknown])
                .collect(),
            case_split: true,
            remaining_case_branches: CASE_BRANCH_BUDGET,
            active_cuts: Vec::new(),
            active_goals: Vec::new(),
        }
    }

    /// `goal` by `ValueEqualityTransport`: under the definitions the prover
    /// holds, its denotation is `Truth`, or is the denotation of a fact. The
    /// kernel recomputes both denotations from the same definitions. Only
    /// when those definitions do not decide it, an arrival's compound
    /// equality may define its otherwise undefined value too: a match arm
    /// that read `window.len` crosses as `length == count - 0`, which the
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
        crate::known::Known::transport_inputs(
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
        } else if let Some(fact) = self.facts.iter().find(|fact| fact.proposition == target) {
            fact.proof(&self.facts)
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
                .or_else(|| self.closed(target))?
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
        crate::known::Known::transport_inputs(
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
        let judgment = if matches!(proposition, Proposition::ScalarIeeeFloatComparison { .. })
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
        let narrower = |bound: Option<Bound>, carrier: IntegerValue| {
            bound
                .map(|bound| bound.literal)
                .filter(|literal| integer.compare(*literal, carrier) != Some(Ordering::Equal))
        };
        let mut lower = narrower(self.bound(value, Direction::Lower), minimum);
        let mut upper = narrower(self.bound(value, Direction::Upper), maximum);
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
            )
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
        if let Some(absurd) = self.refute_closed_mathematical(goal) {
            return Some(absurd);
        }
        let mut orders: Vec<ProofNode> = self
            .facts
            .iter()
            .filter(|fact| fact.axiom.is_none() && order(&fact.proposition).is_some())
            .map(|fact| fact.proof(&self.facts))
            .collect();
        // An equality with a literal is two orders, and either may be the
        // one the value's bounds contradict: a callee's crash route
        // `denominator == 0` assumed at a call that passes 3.
        let equalities: Vec<ProofNode> = self
            .facts
            .iter()
            .filter(|fact| {
                fact.axiom.is_none()
                    && matches!(
                        &fact.proposition,
                        Proposition::Equal(left, right)
                            if left.integer_value().is_some() != right.integer_value().is_some()
                    )
            })
            .map(|fact| fact.proof(&self.facts))
            .collect();
        for equality in equalities {
            orders.extend(weaken(equality.clone()));
            orders.extend(flip(equality).and_then(weaken));
        }
        if let Some(refuted) = orders
            .into_iter()
            .find_map(|known| self.refute(known, goal))
        {
            return Some(refuted);
        }
        let guards: Vec<ProofNode> = self
            .facts
            .iter()
            .filter(|fact| fact.axiom.is_none() && is_branch_fact(&fact.proposition))
            .map(|fact| fact.proof(&self.facts))
            .collect();
        guards
            .into_iter()
            .find_map(|taken| self.refute_guard(taken, goal))
    }

    /// A closed false mathematical assumption also makes a path unreachable.
    /// Retain its exact value equations in the proof; the kernel independently
    /// evaluates the negated comparison and checks the transport back.
    fn refute_closed_mathematical(&mut self, goal: &Proposition) -> Option<ProofNode> {
        for index in 0..self.facts.len() {
            if self.facts[index].axiom.is_some()
                || !matches!(
                    &self.facts[index].proposition,
                    Proposition::IntegerMathEqual(..)
                        | Proposition::IntegerMathLessThan(..)
                        | Proposition::IntegerMathLessOrEqual(..)
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
    fn refute_guard(&mut self, taken: ProofNode, goal: &Proposition) -> Option<ProofNode> {
        let Proposition::Equal(condition, ScalarTerm::Boolean(value)) = &taken.conclusion else {
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
        .ok()?;
        // Unchanged means `condition` has no definition here, and then
        // nothing but the taken fact itself speaks of it.
        if denotation == untaken {
            return None;
        }
        let nested = std::mem::replace(&mut self.case_split, false);
        let premise = self.prove_direct(&denotation);
        self.case_split = nested;
        let untaken = node(
            untaken,
            ProofRule::ValueEqualityTransport {
                premise: Box::new(premise.ok()?),
                equalities: self
                    .equations
                    .iter()
                    .map(|&index| self.facts[index].proof(&self.facts))
                    .collect(),
            },
        );
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
        let disjunctions: Vec<(Vec<Proposition>, ProofNode)> = self
            .facts
            .iter()
            .filter_map(|fact| match &fact.proposition {
                Proposition::Disjunction(parts) => Some((parts.clone(), fact.proof(&self.facts))),
                _ => None,
            })
            .collect();
        for (parts, proof) in disjunctions {
            if self.active_cuts.contains(&proof.conclusion) {
                continue;
            }
            self.active_cuts.push(proof.conclusion.clone());
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
        let local = node(
            part.clone(),
            ProofRule::Assumption {
                index: self.assumptions.len(),
            },
        );
        if let Some(contradiction) = self.refute(local, goal) {
            return Some(contradiction);
        }
        let mut assumptions = self.assumptions.to_vec();
        assumptions.push(part.clone());
        let mut branch = Prover::new(self.context, &assumptions, self.axioms, self.values);
        branch.case_split = self.case_split;
        branch.remaining_case_branches = self.remaining_case_branches;
        branch.active_cuts.clone_from(&self.active_cuts);
        let proof = branch.prove(goal);
        self.remaining_case_branches = branch.remaining_case_branches;
        proof.ok()
    }

    /// `goal` from an alternative that contradicts what is known: chained
    /// with the opposite order it gives a closed false relation, which
    /// `ex_falso` turns into `goal`.
    fn refute(&mut self, local: ProofNode, goal: &Proposition) -> Option<ProofNode> {
        let (strict, left, right) = order(&local.conclusion)?;
        let (left, right) = (left.clone(), right.clone());
        let opposite = if strict {
            Proposition::LessOrEqual(right.clone(), left.clone())
        } else {
            Proposition::LessThan(right.clone(), left.clone())
        };
        let opposite = self.prove_direct(&opposite).ok()?;
        let false_relation = if left.integer_value().is_some() {
            chain(local, opposite).ok()?
        } else if right.integer_value().is_some() {
            chain(opposite, local).ok()?
        } else {
            return None;
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
        let mut assumptions = self.assumptions.to_vec();
        assumptions.push(premise.clone());
        let mut under = Prover::new(self.context, &assumptions, self.axioms, self.values);
        under.case_split = self.case_split;
        under.remaining_case_branches = self.remaining_case_branches;
        under.active_cuts.clone_from(&self.active_cuts);
        let body = under.prove(conclusion);
        self.remaining_case_branches = under.remaining_case_branches;
        let body = body?;
        Ok(node(
            goal.clone(),
            ProofRule::ImplicationIntroduction {
                body: Box::new(body),
            },
        ))
    }

    fn prove_direct(&mut self, goal: &Proposition) -> Result<ProofNode, Unproven> {
        let structural = self.prove_structural(goal);
        if structural.is_ok() {
            return structural;
        }
        self.by_transport(goal)
            .ok_or(structural.err().unwrap_or("no proof"))
    }

    fn prove_structural(&mut self, goal: &Proposition) -> Result<ProofNode, Unproven> {
        if let Some(fact) = self.facts.iter().find(|fact| &fact.proposition == goal) {
            return Ok(fact.proof(&self.facts));
        }
        if let Some(absurd) = self
            .facts
            .iter()
            .find(|fact| fact.proposition == Proposition::Falsehood)
        {
            return Ok(ex_falso(absurd.proof(&self.facts), goal));
        }
        match goal {
            Proposition::Truth => Ok(primitive(Proposition::Truth, PrimitiveJudgment::Truth)),
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
                let mut last = "no disjunct is provable";
                for (index, part) in parts.iter().enumerate() {
                    match self.prove(part) {
                        Ok(disjunct) => {
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
                Err(last)
            }
            Proposition::LessOrEqual(left, right) | Proposition::LessThan(left, right) => {
                if let Some(proof) = self.closed(goal.clone()) {
                    return Ok(proof);
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
                self.arithmetic(left, right)
            }
            Proposition::IntegerMathEqual(..) => self
                .reflexive(goal.clone())
                .or_else(|| self.closed(goal.clone()))
                .ok_or("non-reflexive mathematical equality over open terms"),
            // Strict mathematical order is evaluated only for closed terms;
            // values fixed to literals reach the same rule by transport.
            Proposition::IntegerMathLessThan(..) => self
                .closed(goal.clone())
                .ok_or("mathematical strict order over open terms"),
            Proposition::Atom(_)
            | Proposition::IeeeFloatComparison { .. }
            | Proposition::ByteSequenceEqual { .. }
            | Proposition::StructuralIdentity { .. }
            | Proposition::StructuralCaseMembership { .. }
            | Proposition::ContentConservation(_) => Err("goal form outside this prover"),
            Proposition::FloatMeaningEqual(_) => self.float_meaning(goal),
            Proposition::ScalarIeeeFloatComparison { .. } => self
                .closed(goal.clone())
                .or_else(|| self.ieee_float_order(goal))
                .ok_or("the IEEE order premises do not establish this comparison"),
            Proposition::Implication {
                premise,
                conclusion,
            } => self.implication(premise, conclusion, goal),
        }
    }

    /// `left <= right` or `left < right` from the left side's upper bound and
    /// the right side's lower bound.
    fn compare(
        &mut self,
        left: &ScalarTerm,
        right: &ScalarTerm,
        strict: bool,
    ) -> Result<ProofNode, Unproven> {
        let ScalarType::Integer(integer) = left.scalar_type() else {
            return Err("order of non-integers");
        };
        if let (ScalarTerm::Value { .. }, ScalarTerm::Value { .. }) = (left, right)
            && let Some(proof) = self
                .relation(left, right, strict)
                .or_else(|| (!strict).then(|| self.successor(left, right)).flatten())
        {
            return Ok(proof);
        }
        if let Some(proof) = self.saturating_subtract_order(left, right, strict) {
            return Ok(proof);
        }
        let (left_literal, left_proof) = self.side(left, Direction::Upper)?;
        let (right_literal, right_proof) = self.side(right, Direction::Lower)?;
        let ordered = integer
            .compare(left_literal, right_literal)
            .ok_or("bounds are not comparable")?;
        if ordered == Ordering::Greater || (strict && ordered == Ordering::Equal) {
            return Err("the bounds do not decide the comparison");
        }
        let literal = |value| ScalarTerm::integer(integer, value).map_err(|_| "bound literal");
        let middle_left = match &left_proof {
            Some(_) => literal(left_literal)?,
            None => left.clone(),
        };
        let middle_right = match &right_proof {
            Some(_) => literal(right_literal)?,
            None => right.clone(),
        };
        let middle = if strict {
            Proposition::LessThan(middle_left, middle_right)
        } else {
            Proposition::LessOrEqual(middle_left, middle_right)
        };
        let mut proof = self
            .closed(middle)
            .ok_or("the closed bound relation does not hold")?;
        if let Some(left_proof) = left_proof {
            proof = chain(left_proof, proof)?;
        }
        if let Some(right_proof) = right_proof {
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
        crate::known::Known::transport_inputs(
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
        // The goal with its values unfolded, such as `a +exact 1 <= b`, and
        // that order read over the mathematical integers.
        let unfolded = check_value_equality_denotation(
            self.context,
            &goal,
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
            goal,
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
        let (smaller_equation, smaller_minuend, smaller_subtrahend) =
            self.saturating_subtract_definition(smaller)?;
        let (larger_equation, larger_minuend, larger_subtrahend) =
            self.saturating_subtract_definition(larger)?;
        let minuends = (smaller_minuend, larger_minuend.clone());
        let subtrahends = (larger_subtrahend.clone(), smaller_subtrahend);
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
                smaller: Box::new(smaller_equation),
                larger: Box::new(larger_equation),
                order: Box::new(order),
            },
        ))
    }

    fn saturating_subtract_definition(
        &self,
        subject: &ScalarTerm,
    ) -> Option<(ProofNode, ScalarTerm, ScalarTerm)> {
        let ScalarTerm::Value { id, .. } = subject else {
            return None;
        };
        let index = self.definitions.get(*id)?.as_ref()?;
        let fact = self.facts.get(self.equations[*index])?;
        let Proposition::Equal(
            _,
            ScalarTerm::SaturatingIntegerSubtract {
                scalar_type,
                left,
                right,
            },
        ) = &fact.proposition
        else {
            return None;
        };
        if scalar_type.sign() != semantic_vocabulary::IntegerSign::Unsigned {
            return None;
        }
        Some((
            fact.proof(&self.facts),
            left.as_ref().clone(),
            right.as_ref().clone(),
        ))
    }

    /// `from <= to` (or `<` when `strict`) between two values through order
    /// relations among values, with no literal endpoint: a breadth-first
    /// search whose steps are hypotheses relating two values and exact add or
    /// subtract results ordered against their left operand. Each step is a
    /// node the kernel checks; the path is chained by order transitivity.
    fn relation(&mut self, from: &ScalarTerm, to: &ScalarTerm, strict: bool) -> Option<ProofNode> {
        let mut visited: Vec<[bool; 2]> = vec![[false; 2]; self.values.count()];
        let mut queue: std::collections::VecDeque<(ScalarTerm, ProofNode, bool)> =
            self.steps(from).into_iter().collect();
        while let Some((at, proof, at_strict)) = queue.pop_front() {
            let ScalarTerm::Value { id, .. } = &at else {
                continue;
            };
            let seen = visited.get_mut(value_index(*id))?;
            if seen[usize::from(at_strict)] {
                continue;
            }
            seen[usize::from(at_strict)] = true;
            if &at == to {
                if at_strict || !strict {
                    return Some(if at_strict && !strict {
                        // A strict path proves the non-strict goal.
                        let (_, left, right) = order(&proof.conclusion)?;
                        let conclusion = Proposition::LessOrEqual(left.clone(), right.clone());
                        node(
                            conclusion,
                            ProofRule::IntegerOrderWeakening {
                                relation: Box::new(proof),
                            },
                        )
                    } else {
                        proof
                    });
                }
                continue;
            }
            for (next, step, step_strict) in self.steps(&at) {
                if let Ok(chained) = chain(proof.clone(), step) {
                    queue.push_back((next, chained, at_strict || step_strict));
                }
            }
        }
        None
    }

    /// The order steps leaving `at`: `(next, proof of at R next, strict)`.
    fn steps(&mut self, at: &ScalarTerm) -> Vec<(ScalarTerm, ProofNode, bool)> {
        let mut steps = Vec::new();
        for index in 0..self.facts.len() {
            let proposition = self.facts[index].proposition.clone();
            let axiom = self.facts[index].axiom;
            match &proposition {
                Proposition::LessOrEqual(left, right) | Proposition::LessThan(left, right)
                    if left == at && matches!(right, ScalarTerm::Value { .. }) =>
                {
                    let strict = matches!(proposition, Proposition::LessThan(..));
                    steps.push((right.clone(), self.facts[index].proof(&self.facts), strict));
                }
                Proposition::Equal(left, right)
                    if matches!(left, ScalarTerm::Value { .. })
                        && matches!(right, ScalarTerm::Value { .. })
                        && left != right
                        && (left == at || right == at) =>
                {
                    let proof = self.facts[index].proof(&self.facts);
                    let oriented = if left == at { Ok(proof) } else { flip(proof) };
                    if let Ok(weakened) = oriented.and_then(weaken) {
                        let next = if left == at { right } else { left };
                        steps.push((next.clone(), weakened, false));
                    }
                }
                // `a < a + d` and `a - d < a` when `0 < d`.
                // An arrival can prove the same result equation as an operation
                // axiom. Either equation warrants the checked order step.
                Proposition::Equal(
                    output,
                    ScalarTerm::ExactIntegerAdd {
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
                    }
                }
                Proposition::Equal(
                    output,
                    ScalarTerm::ExactIntegerSubtract {
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
                    }
                }
                // `a / d <= a` when `0 <= a` and `0 < d`.
                Proposition::Equal(output, expression @ ScalarTerm::ExactIntegerDivide { .. })
                    if output == at =>
                {
                    if let Some(axiom) = axiom
                        && let Some((dividend, at_most)) =
                            self.quotient_at_most_dividend(axiom, output, expression)
                    {
                        steps.push((dividend, at_most, false));
                    }
                }
                // `a % d < d` for an unsigned remainder when `0 < d`.
                Proposition::Equal(
                    output,
                    expression @ ScalarTerm::ExactIntegerRemainder { .. },
                ) if output == at => {
                    if let Some(axiom) = axiom
                        && let Some((divisor, below)) =
                            self.remainder_below_divisor(axiom, output, expression)
                    {
                        steps.push((divisor, below, true));
                    }
                }
                _ => {}
            }
        }
        steps
    }

    /// A proof of `0 < term`.
    fn positive(&mut self, integer: IntegerType, term: &ScalarTerm) -> Option<ProofNode> {
        let zero = ScalarTerm::integer(integer, integer.zero()).ok()?;
        self.prove(&Proposition::LessThan(zero, term.clone())).ok()
    }

    /// A literal side is its own bound; a value side takes its best bound.
    fn side(
        &mut self,
        term: &ScalarTerm,
        direction: Direction,
    ) -> Result<(IntegerValue, Option<ProofNode>), Unproven> {
        match term {
            ScalarTerm::Integer { value, .. } => Ok((*value, None)),
            ScalarTerm::Value { id, .. } => {
                let bound = self.bound(*id, direction).ok_or("value has no bound")?;
                Ok((bound.literal, Some(bound.proof)))
            }
            _ => {
                let integer = term
                    .scalar_type()
                    .as_integer()
                    .ok_or("comparison term outside the scalar subset")?;
                // A result's carrier bound can be looser than the arithmetic
                // image, while a result's local facts can sharpen that image.
                let image = self.bound_from_arithmetic_image(term, direction);
                let definition = self.bound_on_definition(term, integer, direction);
                let bound = match (image, definition) {
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
                Ok((bound.literal, Some(bound.proof)))
            }
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
        let slot = self
            .bounds
            .get(value_index(value))?
            .get(direction.index())?;
        match slot {
            Slot::Done(bound) => return bound.clone(),
            // A relation cycle among hypotheses: this branch contributes
            // nothing, the others still do.
            Slot::InProgress => return None,
            Slot::Unknown => {}
        }
        self.bounds[value_index(value)][direction.index()] = Slot::InProgress;
        let best = self.search(value, direction);
        self.bounds[value_index(value)][direction.index()] = Slot::Done(best.clone());
        best
    }

    fn search(&mut self, value: ValueId, direction: Direction) -> Option<Bound> {
        let ScalarType::Integer(integer) = self.values.scalar_type(value)? else {
            return None;
        };
        let subject = ScalarTerm::value(value, ScalarType::Integer(integer));
        let mut candidates = Vec::new();
        candidates.extend(self.carrier(&subject, integer, direction));
        for index in 0..self.facts.len() {
            if let Some(bound) = self.bound_from_fact(index, &subject, integer, direction) {
                candidates.push(bound);
            }
        }
        candidates.into_iter().reduce(|best, next| {
            if direction.prefers(integer.compare(next.literal, best.literal)) {
                next
            } else {
                best
            }
        })
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

    /// A bound on `subject` from one fact, or `None` when the fact says
    /// nothing in this direction.
    fn bound_from_fact(
        &mut self,
        index: usize,
        subject: &ScalarTerm,
        integer: IntegerType,
        direction: Direction,
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
            // Direct bounds.
            (Proposition::LessOrEqual(left, literal), Direction::Upper)
            | (Proposition::LessOrEqual(literal, left), Direction::Lower)
                if left == subject && literal.integer_value().is_some() =>
            {
                let (literal, proof) = self.literal_endpoint(
                    self.facts[index].proof(&self.facts),
                    literal,
                    direction,
                )?;
                Some(Bound { literal, proof })
            }
            (Proposition::LessThan(left, literal), Direction::Upper)
            | (Proposition::LessThan(literal, left), Direction::Lower)
                if left == subject && literal.integer_value().is_some() =>
            {
                let (value, proof) = self.literal_endpoint(
                    self.facts[index].proof(&self.facts),
                    literal,
                    direction,
                )?;
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
                if (left == subject && right.integer_value().is_some())
                    || (right == subject && left.integer_value().is_some()) =>
            {
                // Orient the equality so the weakened `<=` points the right way.
                let proof = self.facts[index].proof(&self.facts);
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
                self.through(self.facts[index].proof(&self.facts), other, direction)
            }
            (Proposition::Equal(left, right), _)
                if matches!(left, ScalarTerm::Value { .. })
                    && matches!(right, ScalarTerm::Value { .. })
                    && left != right
                    && (left == subject || right == subject) =>
            {
                let proof = self.facts[index].proof(&self.facts);
                let wanted_left_is_subject = direction == Direction::Upper;
                let proof = if (left == subject) == wanted_left_is_subject {
                    proof
                } else {
                    flip(proof).ok()?
                };
                let other = if left == subject { right } else { left };
                self.through(weaken(proof).ok()?, other, direction)
            }
            // Result equations: the output's bound from its operands' bounds.
            (Proposition::Equal(output, expression), _) if output == subject => {
                let axiom = axiom?;
                let defined = self.definition(axiom, subject, integer, expression, direction);
                // An unsigned remainder is also below its divisor.
                let below = (direction == Direction::Upper)
                    .then(|| self.remainder_below_divisor(axiom, subject, expression))
                    .flatten()
                    .and_then(|(divisor, relation)| self.through(relation, &divisor, direction));
                // A remainder by a divisor no literal fixes lies between
                // zero and its dividend.
                let within = self.remainder_within_dividend(axiom, subject, expression, direction);
                // A quotient of a nonnegative dividend by a positive divisor
                // is at most the dividend, and so at most its upper bound.
                let quotient = (direction == Direction::Upper)
                    .then(|| self.quotient_at_most_dividend(axiom, subject, expression))
                    .flatten()
                    .and_then(|(dividend, relation)| self.through(relation, &dividend, direction));
                // A remainder by a divisor its own bounds fix to a literal.
                let fixed = self.remainder_by_fixed_divisor(axiom, subject, expression, direction);
                [defined, below, within, quotient, fixed]
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
            _ => None,
        }
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
            _ => return Err("arithmetic goal between two open terms"),
        };
        let image = self.arithmetic_image(term, direction)?;
        self.land(image, term, limit, direction)
    }

    /// Bound the mathematical expression itself. Safety goals and public
    /// equalities use the same checked image; an equality does not authorize
    /// evaluating an overflowing machine operation or changing its policy.
    fn arithmetic_image(
        &mut self,
        term: &IntegerMathTerm,
        direction: Direction,
    ) -> Result<ProofNode, Unproven> {
        let (operation, first, second) = match term {
            IntegerMathTerm::Add(first, second) => (Arithmetic::Add, first, second),
            IntegerMathTerm::Subtract(first, second) => (Arithmetic::Subtract, first, second),
            IntegerMathTerm::Multiply(first, second) => (Arithmetic::Multiply, first, second),
            IntegerMathTerm::ShiftLeft { value, count } => {
                return self.shift_left_image(value, count, direction);
            }
            IntegerMathTerm::MathValue { source_type, value } => {
                return self.value_image(*source_type, *value, term, direction);
            }
            IntegerMathTerm::IntegerLiteral(_) => {
                return self
                    .closed(Proposition::IntegerMathLessOrEqual(
                        term.clone(),
                        term.clone(),
                    ))
                    .ok_or("the kernel refuses the literal bound");
            }
            _ => return Err("arithmetic goal over a single term"),
        };
        let integer = [first.as_ref(), second.as_ref()]
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
        let target = match operation {
            Arithmetic::Add => {
                ScalarTerm::exact_integer_add(integer, first.clone(), second.clone())
            }
            Arithmetic::Subtract => {
                ScalarTerm::exact_integer_subtract(integer, first.clone(), second.clone())
            }
            Arithmetic::Multiply => {
                ScalarTerm::exact_integer_multiply(integer, first.clone(), second.clone())
            }
        }
        .map_err(|_| "operation term")?;
        let interval = self.image(operation, &first, &second, target.clone(), direction);
        let Some(related) = self.related_image(&target, term, direction) else {
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

    /// Separate operand ranges lose constraints such as an addition's remaining
    /// capacity. Offer the retained relations and their exact definitions to the
    /// existing affine checker; it alone decides which bound follows.
    fn related_image(
        &self,
        target: &ScalarTerm,
        term: &IntegerMathTerm,
        direction: Direction,
    ) -> Option<ProofNode> {
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
        for target in std::iter::once(target).chain(exchanged.as_ref()) {
            for fact in &self.facts {
                let Proposition::LessOrEqual(left, right) = &fact.proposition else {
                    continue;
                };
                for root in [left, right] {
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
                    let Ok(conclusion) = map_integer_affine_bound(&form, &fact.proposition) else {
                        continue;
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
                    let image = node(
                        conclusion,
                        ProofRule::IntegerAffineBound {
                            root_bound: Box::new(fact.proof(&self.facts)),
                            witness,
                        },
                    );
                    return Some(node(
                        goal,
                        ProofRule::IntegerOrderSubstitution {
                            relation: Box::new(image),
                            equality: Box::new(equality),
                            endpoint,
                        },
                    ));
                }
            }
        }
        None
    }

    /// The goal `term <= limit` (or `limit <= term`) from `affine`, the
    /// kernel's image bound on `term`: the image itself when it is the goal,
    /// or chained with the closed relation between the image and the limit.
    fn land(
        &self,
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
                self.closed(Proposition::IntegerMathLessOrEqual(
                    image_right.clone(),
                    limit.clone(),
                ))
                .ok_or("the operands' bounds exceed the carrier")?,
            ),
            Direction::Lower => (
                self.closed(Proposition::IntegerMathLessOrEqual(
                    limit.clone(),
                    image_left.clone(),
                ))
                .ok_or("the operands' bounds exceed the carrier")?,
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

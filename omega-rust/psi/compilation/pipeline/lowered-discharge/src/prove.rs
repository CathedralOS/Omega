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
  kernel replays (`ranges`).

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
    check_value_equality_denotation, decide_primitive, map_integer_affine_bound,
};
use semantic_vocabulary::{
    IdTable, IntegerMathTerm, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};
use terminal_check_tables::values::ValueRow;

mod ranges;

/// Why no certificate was built. Diagnostic only.
pub(crate) type Unproven = &'static str;

/// Whether this prover has rules for `goal`'s form at all, decided from the
/// form alone. It mirrors `Prover::prove_direct`: truth and falsehood,
/// conjunctions and disjunctions of covered parts, an implication whose
/// conclusion is covered, integer orders and equalities, and an arithmetic
/// bound of an add, subtract, multiply, left shift or single value against a
/// literal. Anything else the prover can only take from an equal
/// hypothesis, so a failure on it says nothing about the program: it is
/// `UnprovedCause::NoRuleForGoalForm`, published `Incomplete`, not
/// `NoDerivation`.
pub(crate) fn has_rules_for(goal: &Proposition) -> bool {
    match goal {
        Proposition::Truth | Proposition::Falsehood => true,
        Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
            parts.iter().all(has_rules_for)
        }
        // The premise is only assumed; the conclusion is what is proved.
        Proposition::Implication { conclusion, .. } => has_rules_for(conclusion),
        Proposition::LessOrEqual(left, _)
        | Proposition::LessThan(left, _)
        | Proposition::Equal(left, _) => matches!(left.scalar_type(), ScalarType::Integer(_)),
        Proposition::IntegerMathLessOrEqual(left, right) => {
            let bounded = |term: &IntegerMathTerm| {
                matches!(
                    term,
                    IntegerMathTerm::Add(..)
                        | IntegerMathTerm::Subtract(..)
                        | IntegerMathTerm::Multiply(..)
                        | IntegerMathTerm::ShiftLeft { .. }
                        | IntegerMathTerm::MathValue { .. }
                )
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

/// One hypothesis or one conjunct of it, with the node that yields it.
struct Fact {
    proposition: Proposition,
    proof: ProofNode,
    /// The semantic axiom index when this fact is a whole axiom, which the
    /// definition-bound rules cite.
    axiom: Option<usize>,
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
    values: &'a IdTable<ValueId, ValueRow>,
    facts: Vec<Fact>,
    /// The definitions transport substitutes: each proved `v == t` whose
    /// subject `v` is a Boolean value, or whose definition is a literal, with
    /// its proof.
    equations: Vec<(Proposition, ProofNode)>,
    /// `[upper, lower]` per value, indexed by `ValueId - 1`.
    bounds: Vec<[Slot; 2]>,
    /// Whether relations between values are searched (off only to measure
    /// what they buy).
    relations: bool,
    /// Whether a failed goal may be split over a disjunctive fact; off inside
    /// a branch, so splits do not nest.
    case_split: bool,
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

/// The facts transport may substitute: `v == t` for a Boolean value `v` (an
/// operation result equation, a callee guarantee or a block invariant's
/// definition of a join parameter) or for any value defined as a literal.
/// A Boolean literal on the right is a branch fact, not a definition. The
/// kernel keeps the first definition of each subject, as this does.
fn definitions(facts: &[Fact]) -> Vec<(Proposition, ProofNode)> {
    facts
        .iter()
        .filter(|fact| match &fact.proposition {
            Proposition::Equal(subject @ ScalarTerm::Value { .. }, definition) => {
                match definition {
                    ScalarTerm::Boolean(_) => fact.axiom.is_some(),
                    ScalarTerm::Integer { .. } => true,
                    _ => subject.scalar_type() == ScalarType::Boolean,
                }
            }
            _ => false,
        })
        .map(|fact| (fact.proposition.clone(), fact.proof.clone()))
        .collect()
}

/// `c == true` or `c == false` for a Boolean value `c`.
fn is_branch_fact(proposition: &Proposition) -> bool {
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

fn flatten(
    proposition: &Proposition,
    proof: ProofNode,
    axiom: Option<usize>,
    facts: &mut Vec<Fact>,
) {
    if let Proposition::Conjunction(parts) = proposition {
        for (conjunct, part) in parts.iter().enumerate() {
            let eliminated = node(
                part.clone(),
                ProofRule::ConjunctionElimination {
                    conjunction: Box::new(proof.clone()),
                    conjunct,
                },
            );
            flatten(part, eliminated, None, facts);
        }
    }
    facts.push(Fact {
        proposition: proposition.clone(),
        proof,
        axiom,
    });
}

impl<'a> Prover<'a> {
    pub(crate) fn new(
        context: &'a PropositionContext,
        assumptions: &'a [Proposition],
        axioms: &'a [Proposition],
        values: &'a IdTable<ValueId, ValueRow>,
        relations: bool,
    ) -> Self {
        #[cfg(test)]
        SEARCHES.with(|searches| searches.set(searches.get() + 1));
        let mut facts = Vec::new();
        for (index, assumption) in assumptions.iter().enumerate() {
            let proof = node(assumption.clone(), ProofRule::Assumption { index });
            flatten(assumption, proof, None, &mut facts);
        }
        for (index, axiom) in axioms.iter().enumerate() {
            let proof = node(axiom.clone(), ProofRule::SemanticAxiom { index });
            flatten(axiom, proof, Some(index), &mut facts);
        }
        // A branch fact `c == true` says what `c`'s comparison says. Transport
        // it through the definitions of Boolean values and constants, so
        // integer values stay values and their bounds stay findable; the
        // kernel recomputes the same transport (`ValueEqualityTransport`).
        let equations = definitions(&facts);
        if !equations.is_empty() {
            let premises: Vec<(Proposition, ProofNode)> = facts
                .iter()
                .filter(|fact| fact.axiom.is_none() && is_branch_fact(&fact.proposition))
                .map(|fact| (fact.proposition.clone(), fact.proof.clone()))
                .collect();
            for (premise, proof) in premises {
                let Ok(transported) = check_value_equality_denotation(
                    context,
                    &premise,
                    equations.iter().map(|(equation, _)| equation),
                ) else {
                    continue;
                };
                if transported == premise {
                    continue;
                }
                let proof = node(
                    transported.clone(),
                    ProofRule::ValueEqualityTransport {
                        premise: Box::new(proof),
                        equalities: equations.iter().map(|(_, proof)| proof.clone()).collect(),
                    },
                );
                flatten(&transported, proof, None, &mut facts);
            }
        }
        Self {
            context,
            assumptions,
            axioms,
            values,
            facts,
            equations,
            bounds: (0..values.len())
                .map(|_| [Slot::Unknown, Slot::Unknown])
                .collect(),
            relations,
            case_split: true,
        }
    }

    /// `goal` by `ValueEqualityTransport`: under the definitions the prover
    /// holds, its denotation is `Truth`, or is the denotation of a fact. The
    /// kernel recomputes both denotations from the same definitions.
    fn by_transport(&self, goal: &Proposition) -> Option<ProofNode> {
        if self.equations.is_empty() {
            return None;
        }
        let denotation = |proposition: &Proposition| {
            check_value_equality_denotation(
                self.context,
                proposition,
                self.equations.iter().map(|(equation, _)| equation),
            )
            .ok()
        };
        let target = denotation(goal)?;
        let premise = if target == Proposition::Truth {
            primitive(Proposition::Truth, PrimitiveJudgment::Truth)
        } else {
            self.facts
                .iter()
                .find(|fact| fact.proposition == target)?
                .proof
                .clone()
        };
        Some(node(
            goal.clone(),
            ProofRule::ValueEqualityTransport {
                premise: Box::new(premise),
                equalities: self
                    .equations
                    .iter()
                    .map(|(_, proof)| proof.clone())
                    .collect(),
            },
        ))
    }

    fn closed(&self, proposition: Proposition) -> Option<ProofNode> {
        decide_primitive(
            self.context,
            &proposition,
            PrimitiveJudgment::ClosedIntegerRelation,
        )
        .ok()
        .map(|()| primitive(proposition, PrimitiveJudgment::ClosedIntegerRelation))
    }

    /// A certificate whose conclusion is exactly `goal`.
    /// A certificate whose conclusion is exactly `goal`: directly, or by a
    /// case split over a disjunctive fact.
    pub(crate) fn prove(&mut self, goal: &Proposition) -> Result<ProofNode, Unproven> {
        let direct = self.prove_direct(goal);
        if direct.is_ok()
            || !self.case_split
            || !self.relations
            || !matches!(
                goal,
                Proposition::LessOrEqual(..)
                    | Proposition::LessThan(..)
                    | Proposition::Equal(..)
                    | Proposition::IntegerMathLessOrEqual(..)
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
            .ok_or(direct.err().unwrap_or("no proof"))
    }

    /// `goal` at a point no execution reaches: what is known there is
    /// contradictory, so anything follows (`wiki/spec/proofs/contracts.md`,
    /// "Required automatic discharge": order reasoning includes
    /// "contradiction of an unreachable path"). Two searches, both ending in
    /// `ex_falso`: an order fact chained with its opposite into a closed
    /// false relation (`refute`, which needs a literal endpoint), then a
    /// taken guard whose negation is provable (`refute_guard`, which does
    /// not). It is tried last, so a reachable point pays for it only when its
    /// goal is otherwise unproved.
    fn unreachable(&mut self, goal: &Proposition) -> Option<ProofNode> {
        let orders: Vec<ProofNode> = self
            .facts
            .iter()
            .filter(|fact| fact.axiom.is_none() && order(&fact.proposition).is_some())
            .map(|fact| fact.proof.clone())
            .collect();
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
            .map(|fact| fact.proof.clone())
            .collect();
        guards
            .into_iter()
            .find_map(|taken| self.refute_guard(taken, goal))
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
            self.equations.iter().map(|(equation, _)| equation),
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
                    .map(|(_, proof)| proof.clone())
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
                Proposition::Disjunction(parts) => Some((parts.clone(), fact.proof.clone())),
                _ => None,
            })
            .collect();
        'disjunctions: for (parts, proof) in disjunctions {
            let mut branches = Vec::with_capacity(parts.len());
            for part in &parts {
                let Some(branch) = self.branch(part, goal) else {
                    continue 'disjunctions;
                };
                branches.push(branch);
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
        let mut branch = Prover::new(
            self.context,
            &assumptions,
            self.axioms,
            self.values,
            self.relations,
        );
        branch.case_split = false;
        branch.prove(goal).ok()
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
        Some(ex_falso(false_relation, goal))
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
        let mut under = Prover::new(
            self.context,
            &assumptions,
            self.axioms,
            self.values,
            self.relations,
        );
        under.case_split = self.case_split;
        let body = under.prove(conclusion)?;
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
            return Ok(fact.proof.clone());
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
                self.arithmetic(left, right)
            }
            Proposition::Atom(_)
            | Proposition::IntegerMathEqual(..)
            | Proposition::IntegerMathLessThan(..)
            | Proposition::IeeeFloatComparison { .. }
            | Proposition::ScalarIeeeFloatComparison { .. }
            | Proposition::ByteSequenceEqual { .. }
            | Proposition::StructuralIdentity { .. }
            | Proposition::StructuralCaseMembership { .. }
            | Proposition::ContentConservation(_) => Err("goal form outside this prover"),
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
        if self.relations
            && let (ScalarTerm::Value { .. }, ScalarTerm::Value { .. }) = (left, right)
            && let Some(proof) = self.relation(left, right, strict)
        {
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

    /// `from <= to` (or `<` when `strict`) between two values through order
    /// relations among values, with no literal endpoint: a breadth-first
    /// search whose steps are hypotheses relating two values and exact add or
    /// subtract results ordered against their left operand. Each step is a
    /// node the kernel checks; the path is chained by order transitivity.
    fn relation(&mut self, from: &ScalarTerm, to: &ScalarTerm, strict: bool) -> Option<ProofNode> {
        let mut visited: Vec<[bool; 2]> = vec![[false; 2]; self.values.len()];
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
            let proof = self.facts[index].proof.clone();
            let axiom = self.facts[index].axiom;
            match &proposition {
                Proposition::LessOrEqual(left, right) | Proposition::LessThan(left, right)
                    if left == at && matches!(right, ScalarTerm::Value { .. }) =>
                {
                    let strict = matches!(proposition, Proposition::LessThan(..));
                    steps.push((right.clone(), proof, strict));
                }
                Proposition::Equal(left, right)
                    if matches!(left, ScalarTerm::Value { .. })
                        && matches!(right, ScalarTerm::Value { .. })
                        && left != right
                        && (left == at || right == at) =>
                {
                    let oriented = if left == at { Ok(proof) } else { flip(proof) };
                    if let Ok(weakened) = oriented.and_then(weaken) {
                        let next = if left == at { right } else { left };
                        steps.push((next.clone(), weakened, false));
                    }
                }
                // `a < a + d` and `a - d < a` when `0 < d`.
                Proposition::Equal(
                    output,
                    ScalarTerm::ExactIntegerAdd {
                        scalar_type,
                        left,
                        right,
                    },
                ) if axiom.is_some() && left.as_ref() == at => {
                    if let Some(positive) = self.positive(*scalar_type, right) {
                        let conclusion = Proposition::LessThan(at.clone(), output.clone());
                        steps.push((
                            output.clone(),
                            node(
                                conclusion,
                                ProofRule::IntegerAddOrder {
                                    sum: Box::new(proof),
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
                ) if axiom.is_some() && output == at => {
                    if let Some(positive) = self.positive(*scalar_type, right) {
                        let conclusion =
                            Proposition::LessThan(output.clone(), left.as_ref().clone());
                        steps.push((
                            left.as_ref().clone(),
                            node(
                                conclusion,
                                ProofRule::IntegerSubtractOrder {
                                    difference: Box::new(proof),
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
            _ => Err("comparison term outside the scalar subset"),
        }
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
        let ScalarType::Integer(integer) = self.values.get(value)?.scalar_type else {
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
            let better = matches!(
                (integer.compare(next.literal, best.literal), direction),
                (Some(Ordering::Less), Direction::Upper)
                    | (Some(Ordering::Greater), Direction::Lower)
            );
            if better { next } else { best }
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
            Direction::Upper => integer.maximum_value(),
            Direction::Lower => integer.minimum_value(),
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

    /// A bound on `subject` from one fact, or `None` when the fact says
    /// nothing in this direction.
    fn bound_from_fact(
        &mut self,
        index: usize,
        subject: &ScalarTerm,
        integer: IntegerType,
        direction: Direction,
    ) -> Option<Bound> {
        let proof = self.facts[index].proof.clone();
        let axiom = self.facts[index].axiom;
        let proposition = self.facts[index].proposition.clone();
        let literal_bound = |literal: &ScalarTerm, proof: ProofNode| {
            literal
                .integer_value()
                .map(|(_, literal)| Bound { literal, proof })
        };
        match (&proposition, direction) {
            // Direct bounds.
            (Proposition::LessOrEqual(left, literal), Direction::Upper)
            | (Proposition::LessOrEqual(literal, left), Direction::Lower)
                if left == subject && literal.integer_value().is_some() =>
            {
                literal_bound(literal, proof)
            }
            (Proposition::LessThan(left, literal), Direction::Upper)
            | (Proposition::LessThan(literal, left), Direction::Lower)
                if left == subject && literal.integer_value().is_some() =>
            {
                let (_, value) = literal.integer_value()?;
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
                let wanted_left_is_subject = direction == Direction::Upper;
                let proof = if (left == subject) == wanted_left_is_subject {
                    proof
                } else {
                    flip(proof).ok()?
                };
                let literal = if left == subject { right } else { left };
                literal_bound(literal, weaken(proof).ok()?)
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
                self.through(proof, other, direction)
            }
            (Proposition::Equal(left, right), _)
                if matches!(left, ScalarTerm::Value { .. })
                    && matches!(right, ScalarTerm::Value { .. })
                    && left != right
                    && (left == subject || right == subject) =>
            {
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
                [defined, below, within, quotient]
                    .into_iter()
                    .flatten()
                    .reduce(|best, next| {
                        let better = matches!(
                            (integer.compare(next.literal, best.literal), direction),
                            (Some(Ordering::Less), Direction::Upper)
                                | (Some(Ordering::Greater), Direction::Lower)
                        );
                        if better { next } else { best }
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
        let limit = match (&image.conclusion, direction) {
            (Proposition::IntegerMathLessOrEqual(_, limit), Direction::Upper)
            | (Proposition::IntegerMathLessOrEqual(limit, _), Direction::Lower) => limit.clone(),
            _ => return None,
        };
        let IntegerMathTerm::IntegerLiteral(literal) = &limit else {
            return None;
        };
        let literal = literal.as_integer_value(integer)?;
        let output = IntegerMathTerm::MathValue {
            source_type: integer,
            value: *id,
        };
        let (on_output, endpoint) = match direction {
            Direction::Upper => (
                Proposition::IntegerMathLessOrEqual(output.clone(), limit),
                0,
            ),
            Direction::Lower => (
                Proposition::IntegerMathLessOrEqual(limit, output.clone()),
                1,
            ),
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
        let conclusion = Self::oriented(
            subject,
            ScalarTerm::integer(integer, literal).ok()?,
            direction,
        );
        Some(Bound {
            literal,
            proof: node(
                conclusion,
                ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(transported),
                    equality: Box::new(unchanged),
                    endpoint,
                },
            ),
        })
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
        if interval.is_ok() || !self.relations {
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
        let (operation, first, second) = match term {
            IntegerMathTerm::Add(first, second) => (Arithmetic::Add, first, second),
            IntegerMathTerm::Subtract(first, second) => (Arithmetic::Subtract, first, second),
            IntegerMathTerm::Multiply(first, second) => (Arithmetic::Multiply, first, second),
            IntegerMathTerm::ShiftLeft { value, count } => {
                let affine = self.shift_left_image(value, count, direction)?;
                return self.land(affine, term, limit, direction);
            }
            IntegerMathTerm::MathValue { source_type, value } => {
                return self.value_in_carrier(*source_type, *value, term, limit, direction);
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
        let affine = self.image(operation, &first, &second, target, direction)?;
        self.land(affine, term, limit, direction)
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

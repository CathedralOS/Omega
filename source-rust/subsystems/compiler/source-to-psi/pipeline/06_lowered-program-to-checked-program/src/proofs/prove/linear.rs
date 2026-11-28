//! Equalities that follow from the equations among the facts by a linear
//! combination over mathematical integers.
//!
//! The floor leaves out general linear combinations
//! (`wiki/drafts/designs/required_automatic_discharge.md`, "Left out"), and a
//! checker may prove more than the floor. These proofs use only existing
//! certificate rules, so the kernel re-checks every step. Each equation
//! `a == b` used with coefficient `+1` rewrites the current endpoint `T` of an
//! order chain to `b + (T - a)`: the closed-relation primitive orders `T` and
//! `a + (T - a)`, whose ring normal forms agree, and
//! `IntegerMathOrderTranslation` carries `a <= b` to `a + t <= b + t`.
//! Antisymmetry joins the two directions of the goal.

use super::*;
use semantic_vocabulary::IntegerMathLiteral;
use std::collections::BTreeMap;

/// The most equation uses one chain may take, counting a coefficient of `k`
/// as `k` uses.
const MOST_STEPS: u128 = 32;

/// A linear form over mathematical integer atoms: the sum of each atom times
/// its coefficient, plus a constant.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Form {
    atoms: BTreeMap<IntegerMathTerm, i128>,
    constant: i128,
}

impl Form {
    fn of(term: &IntegerMathTerm) -> Option<Self> {
        Some(match term {
            IntegerMathTerm::IntegerLiteral(literal) => Self {
                atoms: BTreeMap::new(),
                constant: literal_value(*literal)?,
            },
            IntegerMathTerm::Add(left, right) => Self::of(left)?.plus(&Self::of(right)?, 1)?,
            IntegerMathTerm::Subtract(left, right) => {
                Self::of(left)?.plus(&Self::of(right)?, -1)?
            }
            IntegerMathTerm::Multiply(left, right) => {
                let (left, right) = (Self::of(left)?, Self::of(right)?);
                if left.atoms.is_empty() {
                    right.scaled(left.constant)?
                } else if right.atoms.is_empty() {
                    left.scaled(right.constant)?
                } else {
                    Self::atom(term)
                }
            }
            _ => Self::atom(term),
        })
    }

    fn atom(term: &IntegerMathTerm) -> Self {
        Self {
            atoms: BTreeMap::from([(term.clone(), 1)]),
            constant: 0,
        }
    }

    /// `self + factor * other`.
    fn plus(mut self, other: &Self, factor: i128) -> Option<Self> {
        for (atom, coefficient) in &other.atoms {
            let entry = self.atoms.entry(atom.clone()).or_insert(0);
            *entry = entry.checked_add(coefficient.checked_mul(factor)?)?;
            if *entry == 0 {
                self.atoms.remove(atom);
            }
        }
        self.constant = self
            .constant
            .checked_add(other.constant.checked_mul(factor)?)?;
        Some(self)
    }

    fn scaled(&self, factor: i128) -> Option<Self> {
        Self::default().plus(self, factor)
    }

    fn is_zero(&self) -> bool {
        self.atoms.is_empty() && self.constant == 0
    }

    /// A term whose ring normal form is this form.
    fn term(&self) -> Option<IntegerMathTerm> {
        let mut term = literal_term(self.constant)?;
        for (atom, coefficient) in &self.atoms {
            let magnitude = literal_term(coefficient.checked_abs()?)?;
            let scaled = if coefficient.unsigned_abs() == 1 {
                atom.clone()
            } else {
                IntegerMathTerm::Multiply(Box::new(magnitude), Box::new(atom.clone()))
            };
            term = if *coefficient > 0 {
                IntegerMathTerm::Add(Box::new(term), Box::new(scaled))
            } else {
                IntegerMathTerm::Subtract(Box::new(term), Box::new(scaled))
            };
        }
        Some(term)
    }
}

fn literal_value(literal: IntegerMathLiteral) -> Option<i128> {
    let magnitude = i128::try_from(literal.magnitude()).ok()?;
    Some(if literal.negative() {
        -magnitude
    } else {
        magnitude
    })
}

fn literal_term(value: i128) -> Option<IntegerMathTerm> {
    IntegerMathLiteral::new(value < 0, value.unsigned_abs())
        .ok()
        .map(IntegerMathTerm::IntegerLiteral)
}

/// An exact rational, kept in lowest terms with a positive denominator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ratio {
    numerator: i128,
    denominator: i128,
}

impl Ratio {
    const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };

    fn new(numerator: i128, denominator: i128) -> Option<Self> {
        if denominator == 0 {
            return None;
        }
        let divisor = gcd(numerator, denominator).max(1);
        let sign = if denominator < 0 { -1 } else { 1 };
        Some(Self {
            numerator: numerator.checked_div(divisor)?.checked_mul(sign)?,
            denominator: denominator.checked_div(divisor)?.checked_mul(sign)?,
        })
    }

    fn integer(value: i128) -> Self {
        Self {
            numerator: value,
            denominator: 1,
        }
    }

    fn is_zero(self) -> bool {
        self.numerator == 0
    }

    fn plus(self, other: Self) -> Option<Self> {
        Self::new(
            self.numerator
                .checked_mul(other.denominator)?
                .checked_add(other.numerator.checked_mul(self.denominator)?)?,
            self.denominator.checked_mul(other.denominator)?,
        )
    }

    fn minus(self, other: Self) -> Option<Self> {
        Self::new(
            self.numerator
                .checked_mul(other.denominator)?
                .checked_sub(other.numerator.checked_mul(self.denominator)?)?,
            self.denominator.checked_mul(other.denominator)?,
        )
    }

    fn times(self, other: Self) -> Option<Self> {
        Self::new(
            self.numerator.checked_mul(other.numerator)?,
            self.denominator.checked_mul(other.denominator)?,
        )
    }

    fn over(self, other: Self) -> Option<Self> {
        Self::new(
            self.numerator.checked_mul(other.denominator)?,
            self.denominator.checked_mul(other.numerator)?,
        )
    }
}

fn gcd(mut left: i128, mut right: i128) -> i128 {
    left = left.checked_abs().unwrap_or(1);
    right = right.checked_abs().unwrap_or(1);
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

/// The coordinates of a form: one per atom and one for the constant.
type Coordinates = BTreeMap<Option<IntegerMathTerm>, Ratio>;

fn coordinates(form: &Form) -> Coordinates {
    let mut coordinates: Coordinates = form
        .atoms
        .iter()
        .map(|(atom, coefficient)| (Some(atom.clone()), Ratio::integer(*coefficient)))
        .collect();
    if form.constant != 0 {
        coordinates.insert(None, Ratio::integer(form.constant));
    }
    coordinates
}

/// `left - factor * right`, dropping zero coordinates.
fn eliminate(left: &mut Coordinates, right: &Coordinates, factor: Ratio) -> Option<()> {
    for (key, value) in right {
        let entry = left.entry(key.clone()).or_insert(Ratio::ZERO);
        *entry = entry.minus(value.times(factor)?)?;
        if entry.is_zero() {
            left.remove(key);
        }
    }
    Some(())
}

/// Integer coefficients `c` with `target == sum(c[i] * equations[i])`, by
/// Gaussian elimination over the rationals. A combination that needs a
/// fraction is not used: scaling the goal is not one of the rules above.
fn combination(target: &Form, equations: &[Form]) -> Option<Vec<i128>> {
    // Each reduced row keeps the combination of original equations it is.
    let mut rows: Vec<(Coordinates, Vec<Ratio>, Option<IntegerMathTerm>)> = Vec::new();
    for (index, equation) in equations.iter().enumerate() {
        let mut row = coordinates(equation);
        let mut uses = vec![Ratio::ZERO; equations.len()];
        uses[index] = Ratio::integer(1);
        for (pivot_row, pivot_uses, pivot) in &rows {
            let Some(value) = row.get(pivot).copied() else {
                continue;
            };
            let factor = value.over(pivot_row[pivot])?;
            eliminate(&mut row, pivot_row, factor)?;
            for (use_, pivot_use) in uses.iter_mut().zip(pivot_uses) {
                *use_ = use_.minus(pivot_use.times(factor)?)?;
            }
        }
        let Some(pivot) = row.keys().find(|key| key.is_some()).cloned() else {
            continue;
        };
        rows.push((row, uses, pivot));
    }
    let mut remaining = coordinates(target);
    let mut solution = vec![Ratio::ZERO; equations.len()];
    for (row, uses, pivot) in &rows {
        let Some(value) = remaining.get(pivot).copied() else {
            continue;
        };
        let factor = value.over(row[pivot])?;
        eliminate(&mut remaining, row, factor)?;
        for (total, use_) in solution.iter_mut().zip(uses) {
            *total = total.plus(use_.times(factor)?)?;
        }
    }
    if !remaining.is_empty() {
        return None;
    }
    solution
        .into_iter()
        .map(|ratio| (ratio.denominator == 1).then_some(ratio.numerator))
        .collect()
}

/// One equation `left == right` among the facts and its proof.
struct Equation {
    left: IntegerMathTerm,
    right: IntegerMathTerm,
    proof: ProofNode,
}

impl Prover<'_> {
    /// `goal`, a fixed integer or mathematical equality, from a linear
    /// combination of the equations among the facts. Value equalities among
    /// the facts are substituted into the goal and every equation first, so
    /// an equality of addresses relates the embedded addresses it names.
    pub(super) fn linear_equality(&mut self, goal: &Proposition) -> Option<ProofNode> {
        if !matches!(
            goal,
            Proposition::Equal(..) | Proposition::IntegerMathEqual(..)
        ) {
            return None;
        }
        let values = self.acyclic_value_equalities();
        let equalities: Vec<Proposition> = values
            .iter()
            .map(|&index| self.facts[index].proposition().clone())
            .collect();
        let denoted = |context: &PropositionContext, proposition: &Proposition| {
            if equalities.is_empty() {
                return Some(proposition.clone());
            }
            check_value_equality_denotation(context, proposition, equalities.iter()).ok()
        };
        let transported = denoted(self.context, goal)?;
        let (left, right) = match &transported {
            Proposition::Equal(left, right)
                if left.scalar_type() == right.scalar_type()
                    && left.scalar_type().as_integer().is_some_and(|integer| {
                        integer.carrier() == IntegerCarrier::Fixed && !integer.is_address()
                    }) =>
            {
                (
                    IntegerMathTerm::embed(left.clone())?,
                    IntegerMathTerm::embed(right.clone())?,
                )
            }
            Proposition::IntegerMathEqual(left, right) => (left.clone(), right.clone()),
            _ => return None,
        };
        let target = Form::of(&left)?.plus(&Form::of(&right)?, -1)?;
        if target.is_zero() {
            return None;
        }
        let equations = self.linear_equations(&values, &denoted);
        if equations.is_empty() {
            return None;
        }
        let forms: Vec<Form> = equations
            .iter()
            .map(|equation| Form::of(&equation.left)?.plus(&Form::of(&equation.right)?, -1))
            .collect::<Option<_>>()?;
        let coefficients = combination(&target, &forms)?;
        let steps: u128 = coefficients.iter().map(|c| c.unsigned_abs()).sum();
        if steps == 0 || steps > MOST_STEPS {
            return None;
        }
        let forward = self.linear_chain(&left, &right, &equations, &coefficients, 1)?;
        let backward = self.linear_chain(&right, &left, &equations, &coefficients, -1)?;
        let proof = match &transported {
            Proposition::Equal(scalar_left, scalar_right) => {
                let assumptions = self.assumptions.len();
                let restated = |proof: ProofNode, left: &ScalarTerm, right: &ScalarTerm| {
                    proof_admission::restate_integer_relation(
                        Proposition::LessOrEqual(left.clone(), right.clone()),
                        proof,
                        assumptions,
                    )
                };
                node(
                    transported.clone(),
                    ProofRule::IntegerOrderAntisymmetry {
                        forward: Box::new(restated(forward, scalar_left, scalar_right)),
                        backward: Box::new(restated(backward, scalar_right, scalar_left)),
                    },
                )
            }
            _ => node(
                transported.clone(),
                ProofRule::IntegerOrderAntisymmetry {
                    forward: Box::new(forward),
                    backward: Box::new(backward),
                },
            ),
        };
        if &transported == goal {
            return Some(proof);
        }
        let equalities = values
            .iter()
            .map(|&index| self.compound_order_fact(index))
            .collect();
        Some(node(
            goal.clone(),
            ProofRule::ValueEqualityTransport {
                premise: Box::new(proof),
                equalities,
            },
        ))
    }

    /// The integer value equalities among the facts that substitute without
    /// a cycle: each defines a subject not yet defined, by a term that does
    /// not reach that subject through the definitions already kept.
    fn acyclic_value_equalities(&self) -> Vec<usize> {
        let mut kept = Vec::new();
        let mut definitions: Vec<(ScalarTerm, Vec<ScalarTerm>)> = Vec::new();
        for index in 0..self.facts.len() {
            let Proposition::Equal(
                subject @ (ScalarTerm::Value { .. } | ScalarTerm::StructuralRead { .. }),
                definition,
            ) = self.facts[index].proposition()
            else {
                continue;
            };
            if subject == definition
                || subject.scalar_type().as_integer().is_none()
                || definitions.iter().any(|(defined, _)| defined == subject)
            {
                continue;
            }
            let mut reached = Vec::new();
            collect_term_subjects(definition, &mut reached);
            let direct = reached.clone();
            let mut cursor = 0;
            while let Some(term) = reached.get(cursor).cloned() {
                cursor += 1;
                if let Some((_, uses)) = definitions.iter().find(|(defined, _)| *defined == term) {
                    for used in uses {
                        if !reached.contains(used) {
                            reached.push(used.clone());
                        }
                    }
                }
            }
            if reached.contains(subject) {
                continue;
            }
            definitions.push((subject.clone(), direct));
            kept.push(index);
        }
        kept
    }

    /// Every mathematical equation among the facts, and every fixed integer
    /// equality read as one, after substituting the value equalities.
    fn linear_equations(
        &mut self,
        values: &[usize],
        denoted: &dyn Fn(&PropositionContext, &Proposition) -> Option<Proposition>,
    ) -> Vec<Equation> {
        let mut equations = Vec::new();
        for index in 0..self.facts.len() {
            let fact = self.facts[index].proposition().clone();
            let (stated, proof) = match &fact {
                Proposition::IntegerMathEqual(..) => (fact.clone(), self.compound_order_fact(index)),
                Proposition::Equal(left, right)
                    if left.scalar_type() == right.scalar_type()
                        && left.scalar_type().as_integer().is_some_and(|integer| {
                            integer.carrier() == IntegerCarrier::Fixed && !integer.is_address()
                        }) =>
                {
                    let Some(lifted) = proof_admission::lift_fixed_integer_relation(&fact) else {
                        continue;
                    };
                    let proof = self.compound_order_fact(index);
                    let proof = proof_admission::restate_integer_relation(
                        lifted.clone(),
                        proof,
                        self.assumptions.len(),
                    );
                    (lifted, proof)
                }
                _ => continue,
            };
            let Some(transported) = denoted(self.context, &stated) else {
                continue;
            };
            let Proposition::IntegerMathEqual(left, right) = &transported else {
                continue;
            };
            if left == right {
                continue;
            }
            let proof = if transported == stated {
                proof
            } else {
                let equalities = values
                    .iter()
                    .map(|&index| self.compound_order_fact(index))
                    .collect();
                node(
                    transported.clone(),
                    ProofRule::ValueEqualityTransport {
                        premise: Box::new(proof),
                        equalities,
                    },
                )
            };
            equations.push(Equation {
                left: left.clone(),
                right: right.clone(),
                proof,
            });
        }
        equations
    }

    /// `from <= to` over mathematical integers, where `from - to` is
    /// `sign` times the combination `coefficients` of `equations`.
    fn linear_chain(
        &self,
        from: &IntegerMathTerm,
        to: &IntegerMathTerm,
        equations: &[Equation],
        coefficients: &[i128],
        sign: i128,
    ) -> Option<ProofNode> {
        let order = |left: &IntegerMathTerm, right: &IntegerMathTerm| {
            Proposition::IntegerMathLessOrEqual(left.clone(), right.clone())
        };
        let compose = |first: ProofNode, second: ProofNode| {
            let (Proposition::IntegerMathLessOrEqual(left, _), Proposition::IntegerMathLessOrEqual(_, right)) =
                (&first.conclusion, &second.conclusion)
            else {
                unreachable!("the chain composes mathematical orders");
            };
            node(
                order(left, right),
                ProofRule::IntegerLessOrEqualTransitivity {
                    left_less_or_equal_middle: Box::new(first),
                    middle_less_or_equal_right: Box::new(second),
                },
            )
        };
        let mut chain = self.closed(order(from, from))?;
        let mut end = from.clone();
        for (equation, coefficient) in equations.iter().zip(coefficients) {
            let uses = coefficient.checked_mul(sign)?;
            if uses == 0 {
                continue;
            }
            // `low <= high` from the equation, oriented so that rewriting
            // `low` to `high` subtracts `low - high` from the endpoint.
            let (low, high, endpoint) = if uses > 0 {
                (&equation.left, &equation.right, 1)
            } else {
                (&equation.right, &equation.left, 0)
            };
            let start = &equation.left;
            let reflexive = self.closed(order(start, start))?;
            let relation = node(
                order(low, high),
                ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(reflexive),
                    equality: Box::new(equation.proof.clone()),
                    endpoint,
                },
            );
            for _ in 0..uses.unsigned_abs() {
                let offset = Form::of(&end)?.plus(&Form::of(low)?, -1)?.term()?;
                let shifted_low = IntegerMathTerm::Add(Box::new(low.clone()), Box::new(offset.clone()));
                let shifted_high =
                    IntegerMathTerm::Add(Box::new(high.clone()), Box::new(offset.clone()));
                let regrouped = self.closed(order(&end, &shifted_low))?;
                let translated = node(
                    order(&shifted_low, &shifted_high),
                    ProofRule::IntegerMathOrderTranslation {
                        relation: Box::new(relation.clone()),
                        addend: offset,
                    },
                );
                chain = compose(compose(chain, regrouped), translated);
                end = shifted_high;
            }
        }
        let closing = self.closed(order(&end, to))?;
        Some(compose(chain, closing))
    }
}

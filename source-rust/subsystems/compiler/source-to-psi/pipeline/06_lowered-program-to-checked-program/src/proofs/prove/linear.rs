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
const MAX_EQUATIONS: usize = 64;

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

/// Each row keeps the equation it represents as a combination of the input
/// equations. Cross multiplication eliminates a pivot without fractions.
struct Row {
    form: Form,
    uses: Vec<i128>,
}

impl Row {
    fn eliminate(&mut self, previous: &Row, pivot: &IntegerMathTerm) -> Option<()> {
        let Some(&value) = self.form.atoms.get(pivot) else {
            return Some(());
        };
        let leading = previous.form.atoms[pivot];
        let (mut a, mut b) = (leading.unsigned_abs(), value.unsigned_abs());
        while b != 0 {
            (a, b) = (b, a % b);
        }
        let divisor = i128::try_from(a).ok()?;
        let scale = leading.checked_div(divisor)?;
        let subtract = value.checked_div(divisor)?;
        self.form = self
            .form
            .scaled(scale)?
            .plus(&previous.form, subtract.checked_neg()?)?;
        for (count, other) in self.uses.iter_mut().zip(&previous.uses) {
            *count = count
                .checked_mul(scale)?
                .checked_sub(other.checked_mul(subtract)?)?;
        }
        Some(())
    }
}

/// Reduce the target against the same pivots as the equations. If it reduces
/// to zero, its column gives the divisor for the other columns. Only whole
/// equation uses can be replayed by translation.
fn combination(target: &Form, equations: &[Form]) -> Option<Vec<i128>> {
    let mut basis: Vec<(IntegerMathTerm, Row)> = Vec::new();
    for (index, form) in equations.iter().chain([target]).enumerate() {
        let mut row = Row {
            form: form.clone(),
            uses: vec![0; equations.len() + 1],
        };
        row.uses[index] = 1;
        for (pivot, previous) in &basis {
            row.eliminate(previous, pivot)?;
        }
        if index == equations.len() {
            if !row.form.is_zero() {
                return None;
            }
            let divisor = row.uses.pop()?.checked_neg()?;
            return row
                .uses
                .into_iter()
                .map(|count| {
                    if count.checked_rem(divisor)? == 0 {
                        count.checked_div(divisor)
                    } else {
                        None
                    }
                })
                .collect();
        }
        if let Some((pivot, _)) = row.form.atoms.first_key_value() {
            basis.push((pivot.clone(), row));
        }
    }
    None
}

/// One mathematical equation and the fact that supplies its proof.
struct Equation {
    left: IntegerMathTerm,
    right: IntegerMathTerm,
    fact: usize,
}

impl Prover<'_> {
    /// Substitute stated equations for the individual factors of two products.
    /// The closed integer kernel compares the expanded products as polynomials.
    /// Each substitution is replayed by congruence, even if a factor is zero.
    pub(super) fn product_equality(&mut self, goal: &Proposition) -> Option<ProofNode> {
        let Proposition::IntegerMathEqual(
            left @ IntegerMathTerm::Multiply(..),
            right @ IntegerMathTerm::Multiply(..),
        ) = goal
        else {
            return None;
        };
        if self
            .facts
            .iter()
            .filter(|fact| {
                matches!(
                    fact.proposition(),
                    Proposition::IntegerMathEqual(..) | Proposition::Equal(..)
                )
            })
            .nth(MAX_EQUATIONS)
            .is_some()
        {
            return None;
        }
        let equations = self.linear_equations(&|_, fact| Some(fact.clone()));
        let mut terms = [left.clone(), right.clone()];
        let mut steps = Vec::new();
        for side in 0..terms.len() {
            for position in 0..2 {
                let IntegerMathTerm::Multiply(first, second) = &terms[side] else {
                    return None;
                };
                let factor = if position == 0 { &**first } else { &**second };
                if !matches!(
                    factor,
                    IntegerMathTerm::MathValue { .. } | IntegerMathTerm::Embed(_)
                ) {
                    continue;
                }
                let replacement = equations.iter().find_map(|eq| {
                    if factor == &eq.left {
                        Some((eq, &eq.right))
                    } else if factor == &eq.right
                        && !matches!(
                            eq.left,
                            IntegerMathTerm::Embed(_) | IntegerMathTerm::MathValue { .. }
                        )
                    {
                        Some((eq, &eq.left))
                    } else {
                        None
                    }
                });
                if let Some((equation, value)) = replacement {
                    let product = if position == 0 {
                        IntegerMathTerm::Multiply(Box::new(value.clone()), second.clone())
                    } else {
                        IntegerMathTerm::Multiply(first.clone(), Box::new(value.clone()))
                    };
                    steps.push((terms.clone(), side, equation));
                    terms[side] = product;
                }
            }
        }
        if steps.is_empty() {
            return None;
        }
        let order = |terms: &[IntegerMathTerm; 2], reverse: usize| {
            Proposition::IntegerMathLessOrEqual(terms[reverse].clone(), terms[1 - reverse].clone())
        };
        let mut forward = self.closed(order(&terms, 0))?;
        let mut backward = self.closed(order(&terms, 1))?;
        for (original, side, equation) in steps.into_iter().rev() {
            let equality = self.linear_premise(equation, &[])?;
            let substitute = |relation: ProofNode, reverse: usize| {
                node(
                    order(&original, reverse),
                    ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(relation),
                        equality: Box::new(equality.clone()),
                        endpoint: side ^ reverse,
                    },
                )
            };
            forward = substitute(forward, 0);
            backward = substitute(backward, 1);
        }
        Some(node(
            goal.clone(),
            ProofRule::IntegerOrderAntisymmetry {
                forward: Box::new(forward),
                backward: Box::new(backward),
            },
        ))
    }

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
        // Limit the matrix and substitution work before constructing any proof.
        // This optional search runs only when there is a mathematical premise.
        let mut count = 0;
        let mut has_math = false;
        for fact in &self.facts {
            match fact.proposition() {
                Proposition::IntegerMathEqual(..) => has_math = true,
                Proposition::Equal(..) => {}
                _ => continue,
            }
            count += 1;
            if count > MAX_EQUATIONS {
                return None;
            }
        }
        if !has_math {
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
        let equations = self.linear_equations(&denoted);
        if equations.is_empty() {
            return None;
        }
        let forms: Vec<Form> = equations
            .iter()
            .map(|equation| Form::of(&equation.left)?.plus(&Form::of(&equation.right)?, -1))
            .collect::<Option<_>>()?;
        let coefficients = combination(&target, &forms)?;
        let steps = coefficients
            .iter()
            .try_fold(0u128, |sum, c| sum.checked_add(c.unsigned_abs()))?;
        if steps == 0 || steps > MOST_STEPS {
            return None;
        }
        let mut used = Vec::new();
        for (equation, coefficient) in equations.iter().zip(coefficients) {
            if coefficient != 0 {
                used.push((
                    equation,
                    coefficient,
                    self.linear_premise(equation, &values)?,
                ));
            }
        }
        let forward = self.linear_chain(&left, &right, &used, 1)?;
        let backward = self.linear_chain(&right, &left, &used, -1)?;
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
        for (index, fact) in self.facts.iter().enumerate() {
            let Proposition::Equal(
                subject @ (ScalarTerm::Value { .. } | ScalarTerm::StructuralRead { .. }),
                definition,
            ) = fact.proposition()
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
        &self,
        denoted: &dyn Fn(&PropositionContext, &Proposition) -> Option<Proposition>,
    ) -> Vec<Equation> {
        self.facts
            .iter()
            .enumerate()
            .filter_map(|(fact, premise)| {
                let stated = premise.proposition();
                let lifted;
                let mathematical = match stated {
                    Proposition::IntegerMathEqual(..) => stated,
                    Proposition::Equal(..) => {
                        lifted = proof_admission::lift_fixed_integer_relation(stated)?;
                        &lifted
                    }
                    _ => return None,
                };
                match denoted(self.context, mathematical)? {
                    Proposition::IntegerMathEqual(left, right) if left != right => {
                        Some(Equation { left, right, fact })
                    }
                    _ => None,
                }
            })
            .collect()
    }

    /// Materialize a premise only after elimination selects it for the certificate.
    fn linear_premise(&mut self, equation: &Equation, values: &[usize]) -> Option<ProofNode> {
        let mut proof = self.compound_order_fact(equation.fact);
        if matches!(proof.conclusion, Proposition::Equal(..)) {
            let lifted = proof_admission::lift_fixed_integer_relation(&proof.conclusion)?;
            proof =
                proof_admission::restate_integer_relation(lifted, proof, self.assumptions.len());
        }
        let goal = Proposition::IntegerMathEqual(equation.left.clone(), equation.right.clone());
        if goal == proof.conclusion {
            return Some(proof);
        }
        Some(node(
            goal,
            ProofRule::ValueEqualityTransport {
                premise: Box::new(proof),
                equalities: values
                    .iter()
                    .map(|&index| self.compound_order_fact(index))
                    .collect(),
            },
        ))
    }

    /// `from <= to` over mathematical integers, where `from - to` is
    /// `sign` times the combination `coefficients` of `equations`.
    fn linear_chain(
        &self,
        from: &IntegerMathTerm,
        to: &IntegerMathTerm,
        equations: &[(&Equation, i128, ProofNode)],
        sign: i128,
    ) -> Option<ProofNode> {
        let order = |left: &IntegerMathTerm, right: &IntegerMathTerm| {
            Proposition::IntegerMathLessOrEqual(left.clone(), right.clone())
        };
        let mut chain = self.closed(order(from, from))?;
        let mut end = from.clone();
        for (equation, coefficient, proof) in equations {
            let uses = coefficient.checked_mul(sign)?;
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
                    equality: Box::new(proof.clone()),
                    endpoint,
                },
            );
            for _ in 0..uses.unsigned_abs() {
                let offset = Form::of(&end)?.plus(&Form::of(low)?, -1)?.term()?;
                let translated = translate(relation.clone(), offset)?;
                let (shifted_low, shifted_high) = ends(&translated)?;
                let regrouped = self.closed(order(&end, shifted_low))?;
                end = shifted_high.clone();
                chain = chain_math(chain_math(chain, regrouped)?, translated)?;
            }
        }
        let closing = self.closed(order(&end, to))?;
        chain_math(chain, closing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elimination_requires_whole_uses_even_with_an_exact_rational_solution() {
        let variable = IntegerMathTerm::MathValue {
            source_type: IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64).unwrap(),
            value: ValueId::new(1).unwrap(),
        };
        let unit = Form::atom(&variable);
        let doubled = unit.scaled(2).unwrap();
        assert_eq!(
            combination(&doubled, std::slice::from_ref(&unit)),
            Some(vec![2])
        );
        assert_eq!(combination(&unit, &[doubled]), None);
    }
}

//! Integer consequences of already captured runtime predicates.
//!
//! Normal-return capture must discharge a selected requirement before importing
//! its guarantees. Its operands may contain Exact arithmetic, so syntactic truth
//! transport alone cannot prove `n - 1 >= 0` from the selected guard `n > 0`.
//! Project observed integer terms into the existing arithmetic engine, with a
//! query-local atom per complete value identity. No source expression is replayed,
//! no caller state is reread, and no future call guarantee enters the premises.
//! Wrapping/saturating operations remain opaque values rather than integer laws;
//! source formation independently checks every executable Exact operation.

use super::super::{BigInt, Engine, Polynomial};
use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::BinaryOperator;

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn judge_observed_predicate(
        &self,
        predicate: StructuralTerm,
        depth: usize,
    ) -> StructuralJudgment {
        match self.judge_equation(
            self.resolve_at(predicate.clone(), depth + 1),
            super::value_predicates::boolean_term(true),
            depth + 1,
        ) {
            StructuralJudgment::Unknown if self.proves_observed_integer_predicate(&predicate) => {
                StructuralJudgment::Proven
            }
            judgment => judgment,
        }
    }

    fn proves_observed_integer_predicate(&self, predicate: &StructuralTerm) -> bool {
        if !self.runtime_body_values || self.case_facts_only {
            return false;
        }
        let mut atoms = Vec::new();
        let Some((operator, left, right)) = self.integer_comparison(predicate, &mut atoms) else {
            return false;
        };
        let mut comparisons = Vec::new();
        for (predicate, truth) in &self.rewrites {
            let truth = self.resolve(truth.clone());
            let StructuralTerm::Constructor { data, case, fields } = truth.semantic_value() else {
                continue;
            };
            if data != "bool" || !fields.is_empty() {
                continue;
            }
            let Some(holds) = language_semantics::const_value::boolean_literal_spelling(case)
            else {
                continue;
            };
            let Some((operator, left, right)) = self.integer_comparison(predicate, &mut atoms)
            else {
                continue;
            };
            comparisons.push((if holds { operator } else { negate(operator) }, left, right));
        }
        let mut engine = Engine::for_explicit_bindings(self.program);
        engine
            .parameter_atoms
            .extend((0..atoms.len()).map(atom_name));
        if !engine.install_hypotheses(comparisons) {
            return false;
        }
        let proves = |difference: Polynomial, minimum| {
            engine.prove_at_least(&engine.substituted(&difference), &BigInt::from_i64(minimum))
        };
        match operator {
            BinaryOperator::Less => proves(right.sub(&left), 1),
            BinaryOperator::LessOrEqual => proves(right.sub(&left), 0),
            BinaryOperator::Greater => proves(left.sub(&right), 1),
            BinaryOperator::GreaterOrEqual => proves(left.sub(&right), 0),
            BinaryOperator::Equal => proves(left.sub(&right), 0) && proves(right.sub(&left), 0),
            BinaryOperator::NotEqual => proves(left.sub(&right), 1) || proves(right.sub(&left), 1),
            _ => false,
        }
    }

    fn integer_comparison(
        &self,
        predicate: &StructuralTerm,
        atoms: &mut Vec<StructuralTerm>,
    ) -> Option<(BinaryOperator, Polynomial, Polynomial)> {
        let StructuralTerm::ScalarBinary {
            references,
            operator,
            meaning,
            left,
            right,
        } = predicate.semantic_value()
        else {
            return None;
        };
        if !matches!(
            operator,
            BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
        ) || meaning
            .iter()
            .any(|(primitive, _)| !primitive.accepts_integer_literal())
        {
            return None;
        }
        let [left, right] = [left, right].map(|operand| self.resolve(operand.as_ref().clone()));
        if ![&left, &right]
            .into_iter()
            .zip(references)
            .all(|(operand, reference)| {
                super::super::structural_terms::scalar_operand_is_observed(
                    self.program,
                    *reference,
                    operand,
                )
            })
        {
            return None;
        }
        Some((
            *operator,
            self.integer_value(&left, atoms, 0)?,
            self.integer_value(&right, atoms, 0)?,
        ))
    }

    fn integer_value(
        &self,
        value: &StructuralTerm,
        atoms: &mut Vec<StructuralTerm>,
        depth: usize,
    ) -> Option<Polynomial> {
        if depth >= 64 {
            return None;
        }
        let value = self.resolve(value.clone());
        let value = value.semantic_value();
        if let StructuralTerm::Integer(integer) = value {
            return Some(Polynomial::constant(integer.clone()));
        }
        if let StructuralTerm::ScalarBinary {
            references,
            operator,
            meaning,
            left,
            right,
        } = value
            && meaning.iter().all(|(primitive, domain)| {
                primitive.accepts_integer_literal() && *domain == ArithmeticDomain::Exact
            })
            && matches!(
                operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            )
        {
            if ![left, right]
                .into_iter()
                .zip(references)
                .all(|(operand, reference)| {
                    super::super::structural_terms::scalar_operand_is_observed(
                        self.program,
                        *reference,
                        operand,
                    )
                })
            {
                return None;
            }
            let left = self.integer_value(left, atoms, depth + 1)?;
            let right = self.integer_value(right, atoms, depth + 1)?;
            return Some(match operator {
                BinaryOperator::Add => left.add(&right),
                BinaryOperator::Subtract => left.sub(&right),
                BinaryOperator::Multiply => left.checked_mul(&right)?,
                _ => unreachable!(),
            });
        }
        let position = if let Some(position) = atoms.iter().position(|atom| atom == value) {
            position
        } else {
            atoms.push(value.clone());
            atoms.len() - 1
        };
        Some(Polynomial::atom(atom_name(position)))
    }
}

fn atom_name(position: usize) -> String {
    format!("\0captured-integer:{position}")
}

fn negate(operator: BinaryOperator) -> BinaryOperator {
    match operator {
        BinaryOperator::Equal => BinaryOperator::NotEqual,
        BinaryOperator::NotEqual => BinaryOperator::Equal,
        BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
        BinaryOperator::LessOrEqual => BinaryOperator::Greater,
        BinaryOperator::Greater => BinaryOperator::LessOrEqual,
        BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
        _ => unreachable!(),
    }
}

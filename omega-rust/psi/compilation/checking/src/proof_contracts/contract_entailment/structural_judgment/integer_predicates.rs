//! Integer consequences of already captured runtime predicates.
//!
//! Normal-return capture must discharge a selected requirement before importing
//! its guarantees. Its operands may contain Exact arithmetic, so syntactic truth
//! transport alone cannot prove `n - 1 >= 0` from the selected guard `n > 0`.
//! Project observed integer terms into the existing arithmetic engine, with a
//! query-local atom per complete value identity. No source expression is replayed,
//! no caller state is reread, and no future call guarantee enters the premises.
//! Wrapping/saturating operations remain opaque values rather than integer laws;
//! Each expanded operation retains its carrier formation obligation until the
//! authenticated premises establish it, independently of the requested goal.

use super::super::{BigInt, Engine, Polynomial};
use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use std::ops::Range;
use typed_trees::expression::BinaryOperator;
use typed_trees::types::PrimitiveType;

type Comparison = (BinaryOperator, Polynomial, Polynomial);

struct ObservedOperation {
    operator: BinaryOperator,
    primitives: [PrimitiveType; 2],
    operands: [Polynomial; 2],
    value: Polynomial,
    children: Range<usize>,
    admitted: bool,
}

/// Query-local preparation is deliberately separate from admission. Only a
/// formed premise may enter the engine; an equation involving division cannot
/// establish its own nonzero divisor. The requested comparison never enters
/// this premise worklist. Children are admitted before their consuming node,
/// including nodes whose normalized value cancelled out of the comparison.
struct IntegerObservations<'program> {
    engine: Engine<'program>,
    atoms: Vec<StructuralTerm>,
    operations: Vec<ObservedOperation>,
    carrier_facts: Vec<Comparison>,
}

impl IntegerObservations<'_> {
    fn admitted(&self, operations: Range<usize>) -> bool {
        self.operations[operations]
            .iter()
            .all(|operation| operation.admitted)
    }

    fn install_premises(&mut self, premises: &mut Vec<(Comparison, Range<usize>)>) -> bool {
        if !self
            .engine
            .install_hypotheses(std::mem::take(&mut self.carrier_facts))
        {
            return false;
        }
        loop {
            let mut changed = false;
            let mut index = 0;
            while index < premises.len() {
                if self.admitted(premises[index].1.clone()) {
                    let (comparison, _) = premises.remove(index);
                    if !self.engine.install_hypotheses(vec![comparison]) {
                        return false;
                    }
                    changed = true;
                } else {
                    index += 1;
                }
            }
            self.engine.refresh_opaque_intervals();
            for index in 0..self.operations.len() {
                let operation = &self.operations[index];
                if !self.admitted(operation.children.clone()) {
                    continue;
                }
                let [left, right] = &operation.operands;
                if operation.admitted
                    || self.engine.integer_operation_lands(
                        operation.operator,
                        operation.primitives,
                        left,
                        right,
                        &operation.value,
                    )
                {
                    changed |= self.engine.install_integer_operation_bounds(
                        operation.operator,
                        left,
                        right,
                        &operation.value,
                    );
                    changed |= !operation.admitted;
                    self.operations[index].admitted = true;
                }
            }
            if !changed {
                return true;
            }
        }
    }
}

impl StructuralJudge<'_> {
    /// Ask about one completed operand, before introducing its cast result.
    /// The address ceiling remains the builtin symbolic model bound; the
    /// hosted producer's current Addr64 representation is not a premise.
    pub(in crate::proof_contracts::contract_entailment) fn observed_integer_fits(
        &self,
        value: StructuralTerm,
        source_reference: typed_trees::types::TypeReferenceHandle,
        target: PrimitiveType,
    ) -> bool {
        let value = StructuralTerm::IntegerEmbedding {
            reference: source_reference,
            value: Box::new(value),
        };
        let (minimum, maximum, upper_operator) = if target == PrimitiveType::Addr {
            let Some(bound) = self
                .program
                .symbols
                .builtin_type_member_symbol(symbols::BuiltinTypeMember::AddressBound)
            else {
                return false;
            };
            (
                BigInt::zero(),
                StructuralTerm::BoundValue(bound),
                BinaryOperator::Less,
            )
        } else {
            let Some((minimum, maximum)) = target.integer_bounds() else {
                return false;
            };
            (
                BigInt::from_i128(minimum),
                StructuralTerm::Integer(BigInt::from_i128(maximum)),
                BinaryOperator::LessOrEqual,
            )
        };
        self.proves_observed_integer_predicate(&StructuralTerm::MathematicalBinary {
            operator: BinaryOperator::LessOrEqual,
            left: Box::new(StructuralTerm::Integer(minimum)),
            right: Box::new(value.clone()),
        }) && self.proves_observed_integer_predicate(&StructuralTerm::MathematicalBinary {
            operator: upper_operator,
            left: Box::new(value),
            right: Box::new(maximum),
        })
    }

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
        let mut observations = IntegerObservations {
            engine: Engine::for_explicit_bindings(self.program),
            atoms: Vec::new(),
            operations: Vec::new(),
            carrier_facts: Vec::new(),
        };
        let Some((operator, left, right)) = self.integer_comparison(predicate, &mut observations)
        else {
            return false;
        };
        let goal_operations = 0..observations.operations.len();
        let mut comparisons = Vec::new();
        for (predicate, truth) in self.rewrites.iter() {
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
            let start = observations.operations.len();
            let Some((operator, left, right)) =
                self.integer_comparison(predicate, &mut observations)
            else {
                continue;
            };
            comparisons.push((
                (if holds { operator } else { negate(operator) }, left, right),
                start..observations.operations.len(),
            ));
        }
        if !observations.install_premises(&mut comparisons)
            || !observations.admitted(goal_operations)
        {
            return false;
        }
        let engine = &observations.engine;
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
        observations: &mut IntegerObservations<'_>,
    ) -> Option<(BinaryOperator, Polynomial, Polynomial)> {
        if let StructuralTerm::MathematicalBinary {
            operator,
            left,
            right,
        } = predicate.semantic_value()
        {
            if !matches!(
                operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            ) {
                return None;
            }
            return Some((
                *operator,
                self.mathematical_value(left, observations, 0)?,
                self.mathematical_value(right, observations, 0)?,
            ));
        }
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
            self.integer_value(&left, meaning[0].0, observations, 0)?,
            self.integer_value(&right, meaning[1].0, observations, 0)?,
        ))
    }

    fn integer_value(
        &self,
        value: &StructuralTerm,
        primitive: PrimitiveType,
        observations: &mut IntegerObservations<'_>,
        depth: usize,
    ) -> Option<Polynomial> {
        if depth >= 64 {
            return None;
        }
        // A completed operand may be described by a public computed-result
        // equation. Preserve its captured leaves before normal arithmetic
        // resolution; no caller expression or private body supplies this value.
        let value = self.resolve(self.completed_value_identity(value.clone()));
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
            && Engine::integer_operation_meaning(*operator, *meaning)
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
            if meaning[0].0 != primitive {
                return None;
            }
            let start = observations.operations.len();
            let left = self.integer_value(left, primitive, observations, depth + 1)?;
            let right = self.integer_value(right, meaning[1].0, observations, depth + 1)?;
            let value = observations.engine.selected_integer_binary_value(
                *operator,
                *meaning,
                left.clone(),
                right.clone(),
            )?;
            observations.operations.push(ObservedOperation {
                operator: *operator,
                primitives: meaning.map(|(primitive, _)| primitive),
                operands: [left, right],
                value: value.clone(),
                children: start..observations.operations.len(),
                admitted: false,
            });
            return Some(value);
        }
        let position =
            if let Some(position) = observations.atoms.iter().position(|atom| atom == value) {
                position
            } else {
                observations.atoms.push(value.clone());
                let position = observations.atoms.len() - 1;
                observations
                    .engine
                    .parameter_atoms
                    .push(atom_name(position));
                position
            };
        let value = Polynomial::atom(atom_name(position));
        // A complete observed value has its selected operand's carrier. These
        // intrinsic bounds apply only to opaque leaves, never to a computation
        // waiting to prove formation or to an anonymous integer literal.
        if primitive == PrimitiveType::Addr {
            let bound = observations.engine.address_model_bound()?;
            observations.carrier_facts.extend([
                (
                    BinaryOperator::GreaterOrEqual,
                    value.clone(),
                    Polynomial::constant(BigInt::zero()),
                ),
                (BinaryOperator::Less, value.clone(), bound),
            ]);
            return Some(value);
        }
        let (minimum, maximum) = primitive.integer_bounds()?;
        observations.carrier_facts.extend([
            (
                BinaryOperator::GreaterOrEqual,
                value.clone(),
                Polynomial::constant(BigInt::from_i128(minimum)),
            ),
            (
                BinaryOperator::LessOrEqual,
                value.clone(),
                Polynomial::constant(BigInt::from_i128(maximum)),
            ),
        ]);
        Some(value)
    }

    fn mathematical_value(
        &self,
        value: &StructuralTerm,
        observations: &mut IntegerObservations<'_>,
        depth: usize,
    ) -> Option<Polynomial> {
        if depth >= 64 {
            return None;
        }
        let value = self.resolve(value.clone());
        match value.semantic_value() {
            StructuralTerm::Integer(value) => Some(Polynomial::constant(value.clone())),
            StructuralTerm::BoundValue(symbol)
                if self.program.symbols.builtin_type_member(*symbol)
                    == Some(symbols::BuiltinTypeMember::AddressBound) =>
            {
                observations.engine.address_model_bound()
            }
            StructuralTerm::IntegerEmbedding { reference, value } => {
                let (primitive, _) = super::super::structural_terms::scalar_operand_meaning(
                    self.program,
                    *reference,
                    value,
                )?;
                self.integer_value(value, primitive, observations, depth + 1)
            }
            StructuralTerm::MathematicalBinary {
                operator,
                left,
                right,
            } => {
                let left = self.mathematical_value(left, observations, depth + 1)?;
                let right = self.mathematical_value(right, observations, depth + 1)?;
                match operator {
                    BinaryOperator::Add => Some(left.add(&right)),
                    BinaryOperator::Subtract => Some(left.sub(&right)),
                    BinaryOperator::Multiply => left.checked_mul(&right),
                    // Quotient and remainder need their own nonzero formation
                    // evidence. They remain opaque, never cancellation laws.
                    _ => None,
                }
            }
            _ => None,
        }
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

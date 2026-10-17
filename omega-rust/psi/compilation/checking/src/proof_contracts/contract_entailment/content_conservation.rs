//! Content-conservation guarantees proved from a checked body.
//!
//! The content normalizer (`value_custody::content_conservation`) already
//! reduces an authored `Owner::content(old(&input)) == separate(...)` ensures
//! to a closed plan: projection terms over structural places with an entry or
//! current revision, and `separate` lists. A bodied callable that authors such
//! a theorem (a selected partition provider) is judged here over the structural
//! judge's sole-arm normal-return value, the same environment that binds
//! locals to their initializer terms.
//!
//! Each projection evaluates the owner's normalized fields on actual captured
//! values. Parameter places rejoin the normal-return owner's exact typed read
//! and entry/current revision; no freshly projected name can replace a read.
//! Result places use the completed returned value. Counted `separate` is the
//! magnitude sum; arithmetic remains the existing selected Exact term algebra.
//!
//! Interval-set algebras have no body judgment here and stand down as
//! Unknown; a provider of such a theorem still needs its own evidence.

use super::arithmetic_judgment::Polynomial;
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use language_semantics::content::{
    ContentAlgebraIdentity, ContentArithmeticOperator, ContentConservationPlan,
    ContentConservationTerm, ContentFieldSegment, ContentPlaceRoot, ContentPlaceSegment,
    ContentProjectionExpression, ContentProjectionPlan, ContentScalarExpression,
    ContentStructuralPlace,
};
use numerics::bignum::BigInt;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;

/// The normalized plan of one authored content-conservation ensures fact on
/// this machine, if the fact is one.
pub(super) fn content_conservation_plan<'plans>(
    machine: &Machine,
    fact: ExpressionHandle,
    plans: &'plans [crate::value_custody::content_conservation::ContentConservationSourcePlan],
) -> Option<&'plans ContentConservationPlan> {
    plans
        .iter()
        .find(|source| source.source_expression == fact && source.plan.owner == machine.symbol)
        .map(|source| &source.plan)
}

/// A statement call to a unit-returning checked machine whose every state
/// body is empty computes nothing and writes nothing: core `drop(value)` is
/// the ordinary case. The captured term environment is unchanged by it, so
/// the sole-arm and unfolding walks step over it like a citation.
pub(super) fn is_empty_body_statement_call(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::statement::TableCall,
) -> bool {
    if !call.receiver.is_empty() || call.discards_result {
        return false;
    }
    let Some(machine) = program.machines().iter().find(|machine| {
        machine.symbol == call.target_symbol
            || program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == call.target_symbol)
    }) else {
        return false;
    };
    machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
        && machine.body_is_present
        && program.machine_states(machine).iter().all(|state| {
            !state.return_type.is_valid()
                && program
                    .statement_table
                    .statements(state.statement_nodes)
                    .is_empty()
        })
}

pub(super) fn judge_content_conservation(
    judge: &StructuralJudge<'_>,
    result: &StructuralTerm,
    plan: &ContentConservationPlan,
    projections: &[ContentProjectionPlan],
) -> StructuralJudgment {
    if !matches!(plan.algebra, ContentAlgebraIdentity::CountedQuantity { .. }) {
        return StructuralJudgment::Unknown;
    }
    let evaluator = Magnitudes {
        judge,
        result,
        projections,
    };
    let (Some(left), Some(right)) = (
        evaluator.magnitude(plan.equation.left()),
        evaluator.magnitude(plan.equation.right()),
    ) else {
        return StructuralJudgment::Unknown;
    };
    match left.sub(&right).constant_value() {
        Some(difference) if difference.is_zero() => StructuralJudgment::Proven,
        Some(_) => StructuralJudgment::Refuted,
        None => StructuralJudgment::Unknown,
    }
}

struct Magnitudes<'judge, 'program> {
    judge: &'judge StructuralJudge<'program>,
    result: &'judge StructuralTerm,
    projections: &'judge [ContentProjectionPlan],
}

impl Magnitudes<'_, '_> {
    fn magnitude(&self, term: &ContentConservationTerm) -> Option<Polynomial> {
        match term {
            ContentConservationTerm::Projection {
                projection_machine,
                subject,
                ..
            } => {
                let projection = self
                    .projections
                    .iter()
                    .find(|projection| projection.machine == *projection_machine)?;
                let ContentProjectionExpression::CountedQuantity { magnitude } =
                    &projection.expression
                else {
                    return None;
                };
                self.scalar(subject, magnitude)
            }
            ContentConservationTerm::Separate(terms) => {
                terms.iter().try_fold(Polynomial::default(), |sum, term| {
                    Some(sum.add(&self.magnitude(term)?))
                })
            }
        }
    }

    fn subject_term(&self, place: &ContentStructuralPlace) -> Option<StructuralTerm> {
        let root = match &place.root {
            ContentPlaceRoot::Parameter { name, .. } => StructuralTerm::Variable(name.clone()),
            ContentPlaceRoot::Result => self.result.clone(),
        };
        place
            .segments
            .iter()
            .try_fold(root, |subject, segment| match segment {
                ContentPlaceSegment::Field(field) => Some(self.field(subject, field)),
                ContentPlaceSegment::Case(_) | ContentPlaceSegment::FixedIndex(_) => None,
            })
    }

    fn field(&self, subject: StructuralTerm, field: &ContentFieldSegment) -> StructuralTerm {
        self.judge.resolve(StructuralTerm::Projection {
            subject: Box::new(subject),
            path: field.name.clone(),
        })
    }

    fn scalar(
        &self,
        subject: &ContentStructuralPlace,
        expression: &ContentScalarExpression,
    ) -> Option<Polynomial> {
        match expression {
            ContentScalarExpression::SubjectField(path)
            | ContentScalarExpression::RuntimeScalarEmbedding(path) => {
                let term = match &subject.root {
                    ContentPlaceRoot::Parameter { symbol, .. } => {
                        let mut segments = subject.segments.clone();
                        segments.extend(path.iter().cloned().map(ContentPlaceSegment::Field));
                        self.judge.captured_parameter_projection(
                            *symbol,
                            &segments,
                            subject.version,
                        )?
                    }
                    ContentPlaceRoot::Result => {
                        let value = self.subject_term(subject)?;
                        path.iter()
                            .fold(value, |value, field| self.field(value, field))
                    }
                };
                Some(integer_polynomial(&self.judge.resolve(term)))
            }
            ContentScalarExpression::Natural(text) => {
                BigInt::from_decimal_str(text).map(Polynomial::constant)
            }
            ContentScalarExpression::Successor(inner) => Some(
                self.scalar(subject, inner)?
                    .add(&Polynomial::constant(BigInt::from_u64(1))),
            ),
            ContentScalarExpression::Arithmetic {
                operator,
                left,
                right,
            } => {
                let left = self.scalar(subject, left)?;
                let right = self.scalar(subject, right)?;
                match operator {
                    ContentArithmeticOperator::Add => Some(left.add(&right)),
                    ContentArithmeticOperator::Subtract => Some(left.sub(&right)),
                    ContentArithmeticOperator::Multiply => left.checked_mul(&right),
                }
            }
        }
    }
}

/// The mathematical value of a resolved scalar term. Exact fixed-width
/// arithmetic is its mathematical result; any other term is one atom.
fn integer_polynomial(term: &StructuralTerm) -> Polynomial {
    match term.semantic_value() {
        StructuralTerm::Integer(value) => Polynomial::constant(value.clone()),
        StructuralTerm::ScalarBinary {
            operator,
            meaning,
            left,
            right,
            ..
        } if meaning.iter().all(|(primitive, domain)| {
            primitive.is_integer() && *domain == numerics::arithmetic::ArithmeticDomain::Exact
        }) =>
        {
            let folded = match operator {
                BinaryOperator::Add => {
                    Some(integer_polynomial(left).add(&integer_polynomial(right)))
                }
                BinaryOperator::Subtract => {
                    Some(integer_polynomial(left).sub(&integer_polynomial(right)))
                }
                BinaryOperator::Multiply => {
                    integer_polynomial(left).checked_mul(&integer_polynomial(right))
                }
                _ => None,
            };
            folded.unwrap_or_else(|| Polynomial::atom(format!("{term:?}")))
        }
        other => Polynomial::atom(format!("{other:?}")),
    }
}

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
//! Result places use the completed returned value. Both algebras use the
//! ordinary captured-integer judge, retaining selected arithmetic formation
//! and observation identities. Counted separation sums magnitudes; interval
//! separation additionally proves natural, ordered bounds and pairwise
//! disjointness before comparing normalized unions. Equal lengths never
//! establish interval equality or confer backing authority.

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
    program: &dyn crate::CheckingInput,
    judge: &StructuralJudge<'_>,
    result: &StructuralTerm,
    plan: &ContentConservationPlan,
    projections: &[ContentProjectionPlan],
) -> StructuralJudgment {
    let evaluator = ContentValues {
        program,
        judge,
        result,
        projections,
    };
    match plan.algebra {
        ContentAlgebraIdentity::CountedQuantity { .. } => {
            let (Some(left), Some(right)) = (
                evaluator.magnitude(plan.equation.left()),
                evaluator.magnitude(plan.equation.right()),
            ) else {
                return StructuralJudgment::Unknown;
            };
            evaluator.compare(BinaryOperator::Equal, &left, &right)
        }
        ContentAlgebraIdentity::IntervalSet { .. } => {
            evaluator.judge_intervals(plan.equation.left(), plan.equation.right())
        }
    }
}

struct ContentValues<'judge, 'program> {
    program: &'judge dyn crate::CheckingInput,
    judge: &'judge StructuralJudge<'program>,
    result: &'judge StructuralTerm,
    projections: &'judge [ContentProjectionPlan],
}

impl ContentValues<'_, '_> {
    fn magnitude(&self, term: &ContentConservationTerm) -> Option<StructuralTerm> {
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
                terms.iter().try_fold(integer(0), |sum, term| {
                    Some(mathematical(
                        BinaryOperator::Add,
                        sum,
                        self.magnitude(term)?,
                    ))
                })
            }
        }
    }

    fn compare(
        &self,
        operator: BinaryOperator,
        left: &StructuralTerm,
        right: &StructuralTerm,
    ) -> StructuralJudgment {
        self.judge
            .judge_observed_predicate(mathematical(operator, left.clone(), right.clone()), 0)
    }

    fn proves(
        &self,
        operator: BinaryOperator,
        left: &StructuralTerm,
        right: &StructuralTerm,
    ) -> bool {
        matches!(
            self.compare(operator, left, right),
            StructuralJudgment::Proven
        )
    }

    fn intervals(
        &self,
        term: &ContentConservationTerm,
        members: &mut Vec<[StructuralTerm; 2]>,
    ) -> Option<()> {
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
                let ContentProjectionExpression::IntervalSet {
                    members: expressions,
                } = &projection.expression
                else {
                    return None;
                };
                for member in expressions {
                    members.push([
                        self.scalar(subject, member.start())?,
                        self.scalar(subject, member.end())?,
                    ]);
                }
            }
            ContentConservationTerm::Separate(terms) => {
                for term in terms {
                    self.intervals(term, members)?;
                }
            }
        }
        Some(())
    }

    /// Sorting is justified by proved separation, not by symbolic display order.
    /// Unknown ordering/overlap leaves the theorem unknown. Empty members have
    /// no position, and adjacent members coalesce independently of input order.
    fn normalized_intervals(
        &self,
        term: &ContentConservationTerm,
    ) -> Option<Vec<[StructuralTerm; 2]>> {
        let mut members = Vec::new();
        self.intervals(term, &mut members)?;
        let mut ordered: Vec<[StructuralTerm; 2]> = Vec::new();
        for [start, end] in members {
            if !self.proves(BinaryOperator::LessOrEqual, &integer(0), &start)
                || !self.proves(BinaryOperator::LessOrEqual, &start, &end)
            {
                return None;
            }
            if self.proves(BinaryOperator::Equal, &start, &end) {
                continue;
            }
            let mut position = 0;
            while position < ordered.len() {
                if self.proves(BinaryOperator::LessOrEqual, &end, &ordered[position][0]) {
                    break;
                }
                if !self.proves(BinaryOperator::LessOrEqual, &ordered[position][1], &start) {
                    return None;
                }
                position += 1;
            }
            ordered.insert(position, [start, end]);
        }
        let mut normalized: Vec<[StructuralTerm; 2]> = Vec::new();
        for [start, end] in ordered {
            if let Some(previous) = normalized.last_mut()
                && self.proves(BinaryOperator::Equal, &previous[1], &start)
            {
                previous[1] = end;
            } else {
                normalized.push([start, end]);
            }
        }
        Some(normalized)
    }

    fn judge_intervals(
        &self,
        left: &ContentConservationTerm,
        right: &ContentConservationTerm,
    ) -> StructuralJudgment {
        let (Some(left), Some(right)) = (
            self.normalized_intervals(left),
            self.normalized_intervals(right),
        ) else {
            return StructuralJudgment::Unknown;
        };
        // A symbolic member might be empty under additional facts. Different
        // normalized lengths therefore mean unknown, not a refuted theorem.
        if left.len() != right.len() {
            return StructuralJudgment::Unknown;
        }
        for (left, right) in left.iter().zip(&right) {
            for (left, right) in left.iter().zip(right) {
                if !self.proves(BinaryOperator::Equal, left, right) {
                    return StructuralJudgment::Unknown;
                }
            }
        }
        StructuralJudgment::Proven
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
    ) -> Option<StructuralTerm> {
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
                let reference =
                    crate::declaration_type_reference(self.program, path.last()?.symbol)?;
                Some(StructuralTerm::IntegerEmbedding {
                    reference,
                    value: Box::new(self.judge.resolve(term)),
                })
            }
            ContentScalarExpression::Natural(text) => {
                BigInt::from_decimal_str(text).map(StructuralTerm::Integer)
            }
            ContentScalarExpression::Successor(inner) => Some(mathematical(
                BinaryOperator::Add,
                self.scalar(subject, inner)?,
                integer(1),
            )),
            ContentScalarExpression::Arithmetic {
                operator,
                left,
                right,
            } => {
                let left = self.scalar(subject, left)?;
                let right = self.scalar(subject, right)?;
                let operator = match operator {
                    ContentArithmeticOperator::Add => BinaryOperator::Add,
                    ContentArithmeticOperator::Subtract => BinaryOperator::Subtract,
                    ContentArithmeticOperator::Multiply => BinaryOperator::Multiply,
                };
                Some(mathematical(operator, left, right))
            }
        }
    }
}

fn integer(value: u64) -> StructuralTerm {
    StructuralTerm::Integer(BigInt::from_u64(value))
}

fn mathematical(
    operator: BinaryOperator,
    left: StructuralTerm,
    right: StructuralTerm,
) -> StructuralTerm {
    StructuralTerm::MathematicalBinary {
        operator,
        left: Box::new(left),
        right: Box::new(right),
    }
}

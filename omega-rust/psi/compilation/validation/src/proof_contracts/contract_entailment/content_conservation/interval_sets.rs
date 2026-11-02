//! Finite interval equality from exact captured endpoints.
//!
//! Separation first proves every member valid and every pair disjoint. Once
//! that holds, a finite half-open union is determined by its signed boundary
//! multiplicities: a start adds one and an end removes one. Cancelling equal
//! boundaries accounts for empty members and adjacency without sorting symbolic
//! coordinates or enumerating their possible orders. Overlap cannot be hidden
//! by cancellation because separation is an independent obligation on each side.

use super::{ContentValues, StructuralJudgment, StructuralTerm};
use language_semantics::content::{
    ContentConservationPlan, ContentConservationTerm, ContentProjectionExpression,
};
use numerics::bignum::BigInt;
use typed_trees::expression::BinaryOperator;

struct Interval {
    start: StructuralTerm,
    end: StructuralTerm,
}

pub(super) fn judge(
    values: &ContentValues<'_, '_>,
    plan: &ContentConservationPlan,
) -> StructuralJudgment {
    let (Some(left), Some(right)) = (
        members(values, plan.equation.left()),
        members(values, plan.equation.right()),
    ) else {
        return StructuralJudgment::Unknown;
    };
    for intervals in [&left, &right] {
        let validity = separated(values, intervals);
        if !matches!(validity, StructuralJudgment::Proven) {
            return validity;
        }
    }
    // Compare left - right. Its positive endpoints are left starts and right
    // ends; its negative endpoints are left ends and right starts. Equality is
    // justified by actual endpoint equations, never total lengths or field names.
    let positive = left
        .iter()
        .map(|interval| &interval.start)
        .chain(right.iter().map(|interval| &interval.end));
    let mut negative = left
        .iter()
        .map(|interval| &interval.end)
        .chain(right.iter().map(|interval| &interval.start))
        .collect::<Vec<_>>();
    for endpoint in positive {
        let Some(index) = negative.iter().position(|other| {
            matches!(
                comparison(values, BinaryOperator::Equal, endpoint, other),
                StructuralJudgment::Proven
            )
        }) else {
            return StructuralJudgment::Unknown;
        };
        negative.swap_remove(index);
    }
    if negative.is_empty() {
        StructuralJudgment::Proven
    } else {
        StructuralJudgment::Unknown
    }
}

fn members(
    values: &ContentValues<'_, '_>,
    term: &ContentConservationTerm,
) -> Option<Vec<Interval>> {
    match term {
        ContentConservationTerm::Projection {
            projection_machine,
            subject,
            ..
        } => {
            let projection = values
                .projections
                .iter()
                .find(|projection| projection.machine == *projection_machine)?;
            let ContentProjectionExpression::IntervalSet { members } = &projection.expression
            else {
                return None;
            };
            members
                .iter()
                .map(|member| {
                    Some(Interval {
                        start: values.scalar(subject, member.start())?,
                        end: values.scalar(subject, member.end())?,
                    })
                })
                .collect()
        }
        ContentConservationTerm::Separate(terms) => {
            let mut intervals = Vec::new();
            for term in terms {
                intervals.extend(members(values, term)?);
            }
            Some(intervals)
        }
    }
}

fn comparison(
    values: &ContentValues<'_, '_>,
    operator: BinaryOperator,
    left: &StructuralTerm,
    right: &StructuralTerm,
) -> StructuralJudgment {
    values.judge.judge_observed_predicate(
        StructuralTerm::MathematicalBinary {
            operator,
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
        },
        0,
    )
}

fn separated(values: &ContentValues<'_, '_>, intervals: &[Interval]) -> StructuralJudgment {
    let zero = StructuralTerm::Integer(BigInt::zero());
    for (index, interval) in intervals.iter().enumerate() {
        for (left, right) in [(&zero, &interval.start), (&interval.start, &interval.end)] {
            let validity = comparison(values, BinaryOperator::LessOrEqual, left, right);
            if !matches!(validity, StructuralJudgment::Proven) {
                return validity;
            }
        }
        if matches!(
            comparison(
                values,
                BinaryOperator::Equal,
                &interval.start,
                &interval.end
            ),
            StructuralJudgment::Proven
        ) {
            continue;
        }
        for earlier in &intervals[..index] {
            if matches!(
                comparison(values, BinaryOperator::Equal, &earlier.start, &earlier.end),
                StructuralJudgment::Proven
            ) || matches!(
                comparison(
                    values,
                    BinaryOperator::LessOrEqual,
                    &earlier.end,
                    &interval.start
                ),
                StructuralJudgment::Proven
            ) || matches!(
                comparison(
                    values,
                    BinaryOperator::LessOrEqual,
                    &interval.end,
                    &earlier.start
                ),
                StructuralJudgment::Proven
            ) {
                continue;
            }
            return StructuralJudgment::Unknown;
        }
    }
    StructuralJudgment::Proven
}

//! Range constraints concern the produced rank, not its subject's carrier.

use super::{DecreaseMeasure, RankingOrder};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;

mod endpoints;
mod relational;

pub(super) struct RangeProof {
    /// Only the relational tier also checks strict decrease on every exact
    /// self-edge. Static membership must still pass the existing descent owner.
    pub strict_decrease_proven: bool,
}

pub(super) fn check(
    program: &TypedTrees,
    machine: &Machine,
    range: &language_semantics::RankRange,
    order: &RankingOrder,
    measure: DecreaseMeasure,
) -> Result<RangeProof, String> {
    if proves_range(program, machine, order, measure) == Some(true) {
        return Ok(RangeProof {
            strict_decrease_proven: false,
        });
    }
    if relational::prove(program, machine, measure, order) {
        return Ok(RangeProof {
            strict_decrease_proven: true,
        });
    }
    // Diagnostics use the retained expression tree. Normalized witness labels
    // deliberately cover a smaller subset and may spell arithmetic as "value".
    let retained = program
        .ranking_expression_custody_for(machine.symbol)
        .and_then(|custody| custody.rank_range)
        .and_then(|handle| match program.expression_table.expression(handle) {
            ExpressionNode::Range(range) => Some((
                program.expression_table.display_name(range.start),
                program.expression_table.display_name(range.end),
                range.end_inclusive,
            )),
            _ => None,
        });
    let (floor, ceiling, inclusive) = retained.unwrap_or_else(|| {
        (
            range.floor.clone(),
            range.ceiling.clone(),
            range.ceiling_inclusive,
        )
    });
    let separator = if inclusive { "..=" } else { ".." };
    Err(format!(
        "cannot prove rank range `{floor}{separator}{ceiling}` for the rank produced by the selected view",
    ))
}

/// Static membership alone says nothing about progress. If the usual descent
/// recognizer declines, reuse the complete relational entry-and-edge judgment;
/// never promote its membership result without strict decrease and pinning.
pub(super) fn proves_relational_decrease(
    program: &TypedTrees,
    machine: &Machine,
    order: &RankingOrder,
    measure: DecreaseMeasure,
) -> bool {
    relational::prove(program, machine, measure, order)
}

fn proves_range(
    program: &TypedTrees,
    machine: &Machine,
    order: &RankingOrder,
    measure: DecreaseMeasure,
) -> Option<bool> {
    // Source custody, not normalized display strings, selects the endpoints.
    let custody = program.ranking_expression_custody_for(machine.symbol)?;
    let ExpressionNode::Range(range) = program.expression_table.expression(custody.rank_range?)
    else {
        return None;
    };
    // Named-state transport needs a proof of the subject and endpoint bounds
    // at each exact arrival; entry parameter declarations cannot stand in for it.
    if program.machine_states(machine).len() != 1 {
        return None;
    }
    let (rank, pinned_bound) = match (order, measure) {
        (
            RankingOrder::NatDescending | RankingOrder::CustomNatDescending,
            DecreaseMeasure::Single(subject),
        ) => (bounds(program, machine, subject)?, None),
        (RankingOrder::CustomStructView { field_type, .. }, DecreaseMeasure::Single(_)) => {
            // Order resolution already joined the exact parameter, field and
            // nominal carrier. Its store-enforced type range bounds the
            // produced rank; reconstruction and descent remain separate checks.
            let (low, high) = validation::enforced_integer_type_bounds(program, *field_type)?;
            (
                Bounds {
                    low: i128::from(low),
                    high: i128::from(high),
                },
                None,
            )
        }
        (RankingOrder::IncreasingTo(limit), DecreaseMeasure::Distance { lower, upper })
            if *limit == upper =>
        {
            let subject = bounds(program, machine, lower)?;
            let bound = bounds(program, machine, upper)?;
            if subject.low < 0 || bound.low < 0 {
                return None;
            }
            // The view produces natural distance, including zero after the
            // cursor reaches its limit; it does not rank the cursor itself.
            let rank = Bounds {
                low: (bound.low - subject.high).max(0),
                high: (bound.high - subject.low).max(0),
            };
            (rank, Some((upper, subject.low, bound.low)))
        }
        _ => return None,
    };
    if rank.low < 0 {
        return None;
    }
    let floor = endpoint_bounds(program, machine, range.start, pinned_bound)?;
    let ceiling = endpoint_bounds(program, machine, range.end, pinned_bound)?;
    let floor_proven = floor.high <= rank.low;
    let ceiling_proven = if range.end_inclusive {
        rank.high <= ceiling.low
    } else {
        rank.high < ceiling.low
    };
    let view_ceiling_proven = pinned_bound.is_some_and(|(bound, subject_floor, bound_floor)| {
        same_parameter(program, range.end, bound)
            && (range.end_inclusive || (subject_floor > 0 && bound_floor > 0))
    });
    Some(floor_proven && (ceiling_proven || view_ceiling_proven))
}

#[derive(Clone, Copy)]
struct Bounds {
    low: i128,
    high: i128,
}

fn endpoint_bounds(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
    pinned_bound: Option<(ExpressionHandle, i128, i128)>,
) -> Option<Bounds> {
    // The IncreasingTo edge proof already pins this exact bound. Other scalar
    // endpoints need their own occurrence and write-preservation judgment.
    if let Some((bound, _, _)) = pinned_bound
        && same_parameter(program, expression, bound)
    {
        return bounds(program, machine, expression);
    }
    endpoints::pinned_expression_bounds(program, machine, expression)
}

fn same_parameter(program: &TypedTrees, left: ExpressionHandle, right: ExpressionHandle) -> bool {
    match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            left.symbol.is_valid()
                && left.head_symbol == left.symbol
                && right.head_symbol == right.symbol
                && left.symbol == right.symbol
        }
        _ => false,
    }
}

fn bounds(program: &TypedTrees, machine: &Machine, expression: ExpressionHandle) -> Option<Bounds> {
    let state = program.machine_states(machine).first()?;
    if let Some((low, high)) =
        validation::immutable_integer_expression_bounds(program, machine, state, expression)
    {
        return (low <= high).then_some(Bounds {
            low: i128::from(low),
            high: i128::from(high),
        });
    }
    // The shared interval engine uses i64 endpoints and cannot bound an
    // unrestricted u64 above. Widen to its carrier, never invent a range fact.
    let ExpressionNode::Name(name) = program.expression_table.expression(expression) else {
        return None;
    };
    let parameter = program.state_parameters(state).iter().find(|parameter| {
        name.symbol.is_valid()
            && name.head_symbol == name.symbol
            && parameter.symbol == name.symbol
            && !parameter.is_self
            && !parameter.is_mutable
            && !parameter.is_const
    })?;
    (program.primitive_type_reference(parameter.type_reference)
        == Some(typed_trees::types::PrimitiveType::U64))
    .then_some(Bounds {
        low: 0,
        high: i128::from(u64::MAX),
    })
}

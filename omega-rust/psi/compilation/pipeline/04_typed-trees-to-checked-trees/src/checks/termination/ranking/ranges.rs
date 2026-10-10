//! Range constraints concern the produced rank, not its subject's carrier.

use super::{DecreaseMeasure, RankingOrder};
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::Machine;

mod relational;

pub(in crate::checks::termination) use relational::RankArrivalAdmission;

pub(super) fn check(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    range: &language_semantics::RankRange,
    order: &RankingOrder,
    measure: DecreaseMeasure,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    accept: &mut dyn FnMut(&RankArrivalAdmission) -> bool,
) -> Result<RankArrivalAdmission, String> {
    let failure = match relational::admit(program, machine, measure, order, call_frames, accept) {
        Ok(arrival) => return Ok(arrival),
        Err(failure) => failure,
    };
    if matches!(failure, relational::RankArrivalFailure::StrictDecrease) {
        return Err("cannot prove strict rank decrease on a recurrent arrival".to_string());
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

/// An unbounded natural view uses the same complete relational source owner.
/// Optional authored ranges add formation and pinning obligations to that
/// owner; interval membership alone never produces an arrival witness.
pub(super) fn admit_relational_decrease(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    order: &RankingOrder,
    measure: DecreaseMeasure,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    accept: &mut dyn FnMut(&RankArrivalAdmission) -> bool,
) -> Option<RankArrivalAdmission> {
    relational::admit(program, machine, measure, order, call_frames, accept).ok()
}

//! Apply the selected machine's retry condition to this call's borrowed storage.

use super::super::progress::ProgressFailure;
use super::super::spin::{CellUpdate, Observation, SpinCondition, SpinWait, WaitRule};
use super::activation::Bindings;
use super::storage::{Storage, Stored};
use semantic_vocabulary::Proposition;
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralPathSegment};

pub(super) fn apply(
    wait: &SpinWait,
    bindings: &Bindings,
    memory: &mut Storage,
    arguments: &[StructuralArgument],
    requirements: &[Proposition],
) -> Option<Result<(), ProgressFailure>> {
    let locate = |parameter: usize, path: &[StructuralPathSegment]| {
        let actual = arguments.get(parameter)?;
        if actual.access != StructuralAccess::SharedBorrow {
            return None;
        }
        let base = memory.locate(&bindings.places, actual.place, &actual.path)?;
        memory.project(base, path)
    };
    match &wait.0 {
        WaitRule::Cell(cell) => {
            let location = locate(cell.parameter, &cell.path)?;
            let stored = memory.get(&location);
            let cause = failure(&stored, cell.while_value, cell.update, requirements);
            if cause.is_none() && cell.update != CellUpdate::Observe {
                memory.set(&location, stored.after_update(cell.update));
            }
            Some(cause.map_or(Ok(()), Err))
        }
        WaitRule::Observations(predicate) => {
            let retry = predicate.evaluate(|subject, test| {
                let Some(location) = locate(subject.parameter, &subject.path) else {
                    return Observation::Untracked;
                };
                match failure(
                    &memory.get(&location),
                    test,
                    CellUpdate::Observe,
                    requirements,
                ) {
                    None => Observation::Known(false),
                    Some(ProgressFailure::Deadlock) => Observation::Known(true),
                    Some(ProgressFailure::Unproved) => Observation::Unproved,
                }
            });
            match retry {
                Observation::Known(false) => Some(Ok(())),
                Observation::Known(true) => Some(Err(ProgressFailure::Deadlock)),
                Observation::Unproved => Some(Err(ProgressFailure::Unproved)),
                Observation::Untracked => None,
            }
        }
    }
}

fn failure(
    stored: &Stored,
    condition: SpinCondition,
    update: CellUpdate,
    requirements: &[Proposition],
) -> Option<ProgressFailure> {
    let mut all_block = true;
    let mut can_block = false;
    let visited = stored.visit_scalars(condition.comparison.pivot.scalar_type(), |value| {
        let cause = condition.failure(Some(value), update, requirements);
        all_block &= cause == Some(ProgressFailure::Deadlock);
        can_block |= cause.is_some();
    });
    if visited.is_none() {
        return condition.failure(None, update, requirements);
    }
    can_block.then_some(if all_block {
        ProgressFailure::Deadlock
    } else {
        ProgressFailure::Unproved
    })
}

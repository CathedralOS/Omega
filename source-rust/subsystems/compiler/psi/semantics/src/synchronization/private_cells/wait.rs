//! Apply the selected machine's retry condition to this call's borrowed storage.

use super::super::progress::{ProgressFailure, ProgressScope};
use super::super::spin::{CellUpdate, Observation, SpinCondition, SpinWait, WaitRule};
use super::activation::Bindings;
use super::storage::{Storage, Stored};
use semantic_vocabulary::Proposition;
use terminal_psi::{StructuralArgument, StructuralPathSegment};

pub(super) fn apply(
    wait: &SpinWait,
    bindings: &Bindings,
    memory: &mut Storage,
    arguments: &[StructuralArgument],
    requirements: &[Proposition],
    scope: ProgressScope,
) -> Result<(), ProgressFailure> {
    let locate = |parameter: usize, path: &[StructuralPathSegment]| {
        let actual = arguments.get(parameter)?;
        if !matches!(actual.access, terminal_psi::reading_borrow_pair_pattern!()) {
            return None;
        }
        let base = memory.locate(&bindings.places, actual.place, &actual.path)?;
        memory.project(base, path)
    };
    match &wait.0 {
        WaitRule::Cell(cell) => {
            let location = locate(cell.subject.parameter, &cell.subject.path).ok_or(ProgressFailure::Unproved)?;
            let stored = memory.get(&location);
            let cause = classify(
                memory,
                &location,
                cell.while_value,
                cell.update,
                requirements,
            );
            if let Some(cause) = cause
                && !(cause == ProgressFailure::UnprovedExternal
                    && scope == ProgressScope::OpenInterface)
            {
                return Err(cause);
            }
            if cell.update != CellUpdate::Observe {
                memory.set(&location, stored.after_update(cell.update));
            }
            Ok(())
        }
        WaitRule::Observations(predicate) => {
            let retry = predicate.evaluate(|subject, test| {
                let Some(location) = locate(subject.parameter, &subject.path) else {
                    return Observation::Unproved;
                };
                match classify(memory, &location, test, CellUpdate::Observe, requirements) {
                    None => Observation::Known(false),
                    Some(ProgressFailure::Deadlock) => Observation::Known(true),
                    Some(ProgressFailure::Unproved | ProgressFailure::UnprovedImport) => Observation::Unproved,
                    Some(ProgressFailure::UnprovedExternal) => Observation::Supplied,
                }
            });
            match retry {
                Observation::Known(false) => Ok(()),
                Observation::Known(true) => Err(ProgressFailure::Deadlock),
                Observation::Unproved => Err(ProgressFailure::Unproved),
                Observation::Supplied if scope == ProgressScope::OpenInterface => Ok(()),
                Observation::Supplied => Err(ProgressFailure::UnprovedExternal),
            }
        }
    }
}

fn classify(
    memory: &Storage,
    location: &super::storage::Location,
    condition: SpinCondition,
    update: CellUpdate,
    requirements: &[Proposition],
) -> Option<ProgressFailure> {
    let assess = |stored: &Stored| failure(stored, condition, update, requirements);
    let cause = assess(&memory.get(location))?;
    Some(
        if memory.only_caller_can_block(location, |stored| assess(stored).is_some()) {
            ProgressFailure::UnprovedExternal
        } else {
            cause
        },
    )
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

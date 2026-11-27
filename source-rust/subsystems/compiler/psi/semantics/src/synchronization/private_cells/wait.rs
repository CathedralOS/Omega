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
            let location = locate(cell.subject.parameter, &cell.subject.path)
                .ok_or(ProgressFailure::Unproved)?;
            let stored = memory.get(&location);
            let observation = classify(
                memory,
                &location,
                cell.while_value,
                cell.update,
                requirements,
            );
            require_progress(observation, scope)?;
            if cell.update != CellUpdate::Observe {
                memory.set(&location, stored.after_update(cell.update));
            }
            Ok(())
        }
        WaitRule::Observations(predicate) => {
            let retry = predicate.evaluate(|subject, test| {
                let Some(location) = locate(subject.parameter, &subject.path) else {
                    return (Observation::Unproved, None);
                };
                let observation =
                    classify(memory, &location, test, CellUpdate::Observe, requirements);
                let stable = (!location.includes_shared()).then_some(location);
                (observation, stable)
            });
            require_progress(retry, scope)
        }
    }
}

fn require_progress(observation: Observation, scope: ProgressScope) -> Result<(), ProgressFailure> {
    match observation {
        Observation::Known(false) | Observation::PendingProvider => Ok(()),
        Observation::Known(true) => Err(ProgressFailure::Deadlock),
        Observation::Unproved => Err(ProgressFailure::Unproved),
        Observation::Supplied if scope == ProgressScope::OpenInterface => Ok(()),
        Observation::Supplied => Err(ProgressFailure::UnprovedExternal),
    }
}

fn classify(
    memory: &Storage,
    location: &super::storage::Location,
    condition: SpinCondition,
    update: CellUpdate,
    requirements: &[Proposition],
) -> Observation {
    let assess = |stored: &Stored| observation(stored, condition, update, requirements);
    let result = assess(&memory.get(location));
    if result != Observation::Known(false)
        && memory.only_caller_can_block(location, |stored| {
            assess(stored) != Observation::Known(false)
        })
    {
        Observation::Supplied
    } else {
        result
    }
}

fn observation(
    stored: &Stored,
    condition: SpinCondition,
    update: CellUpdate,
    requirements: &[Proposition],
) -> Observation {
    let mut all_block = true;
    let mut can_block = false;
    let visited = stored.visit_scalars(condition.comparison.pivot.scalar_type(), |value| {
        let cause = condition.failure(Some(value), update, requirements);
        all_block &= cause == Some(ProgressFailure::Deadlock);
        can_block |= cause.is_some();
    });
    let unresolved_can_block = condition.failure(None, update, requirements).is_some();
    if stored.includes_pending_provider() && unresolved_can_block {
        return if can_block {
            Observation::Unproved
        } else {
            Observation::PendingProvider
        };
    }
    if visited.is_none() {
        if unresolved_can_block {
            Observation::Unproved
        } else {
            Observation::Known(false)
        }
    } else if all_block && can_block {
        Observation::Known(true)
    } else if can_block {
        Observation::Unproved
    } else {
        Observation::Known(false)
    }
}

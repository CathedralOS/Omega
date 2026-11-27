//! Apply the selected machine's retry condition to this call's borrowed storage.

use super::super::progress::{ProgressFailure, ProgressScope, ScalarFact};
use super::super::spin::Subject;
use super::super::spin::{CellUpdate, Observation, Operand, SpinCondition, SpinWait, WaitRule};
use super::activation::Bindings;
use super::storage::{Storage, Stored};
use semantic_vocabulary::Proposition;
use terminal_psi::{StructuralArgument, StructuralPathSegment};

pub(super) fn apply(
    wait: &SpinWait,
    bindings: &Bindings,
    memory: &mut Storage,
    arguments: &[StructuralArgument],
    scalar: impl Fn(usize) -> Option<ScalarFact>,
    requirements: &[Proposition],
    scope: ProgressScope,
) -> Result<(), ProgressFailure> {
    let resolve = |operand: Operand| match operand {
        Operand::Literal(value) => Some(ScalarFact::Literal(value)),
        Operand::Parameter {
            position,
            scalar_type,
        } => scalar(position).filter(|fact| fact.ty() == scalar_type),
    };
    let locate = |subject: &Subject| {
        let actual = arguments.get(subject.parameter)?;
        if !matches!(actual.access, terminal_psi::reading_borrow_pair_pattern!()) {
            return None;
        }
        let mut location = memory.locate(bindings, actual.place, &actual.path)?;
        if let Some(path) = subject.fixed_path() {
            return memory.project(location, &path);
        }
        let mut indices = subject.indices();
        for part in &subject.path {
            location = match part {
                StructuralPathSegment::FixedRange { start: 0, end } => {
                    Storage::index(location, resolve(indices.next()?), *end)?
                }
                part => memory.project(location, std::slice::from_ref(part))?,
            };
        }
        Some(location)
    };
    let pivot = |mut test: SpinCondition| {
        let mut input = None;
        if let Some(mask) = test.comparison.mask
            && let Some(ScalarFact::Literal(value)) = resolve(mask)
        {
            test.comparison.mask = Some(Operand::Literal(value));
        }
        match resolve(test.comparison.pivot) {
            Some(ScalarFact::Literal(value)) => test.comparison.pivot = Operand::Literal(value),
            Some(ScalarFact::EntryInput(formal)) => input = Some(formal),
            None => {}
        }
        (test, input)
    };
    match &wait.0 {
        WaitRule::Cell(cell) => {
            let location = locate(&cell.subject).ok_or(ProgressFailure::Unproved)?;
            let stored = memory.get(&location);
            let (condition, input) = pivot(cell.while_value);
            let update = cell.update.try_map(resolve);
            let observation = classify(memory, &location, condition, input, update, requirements);
            require_progress(observation, scope)?;
            if update != Some(CellUpdate::Observe) {
                memory.set(
                    &location,
                    update
                        .map(|update| stored.after_update(update))
                        .unwrap_or_default(),
                );
            }
            Ok(())
        }
        WaitRule::Observations(predicate) => {
            let retry = predicate.evaluate(|subject, test| {
                let Some(location) = locate(subject) else {
                    return (Observation::Unproved, None);
                };
                let (test, input) = pivot(test);
                let observation = classify(
                    memory,
                    &location,
                    test,
                    input,
                    Some(CellUpdate::Observe),
                    requirements,
                );
                let stable = (!location.includes_shared()
                    && location.stable()
                    && test.comparison.pivot.literal().is_some())
                .then_some((location, test));
                (observation, stable)
            });
            require_progress(retry, scope)
        }
    }
}

pub(super) fn require_progress(
    observation: Observation,
    scope: ProgressScope,
) -> Result<(), ProgressFailure> {
    match observation {
        Observation::Known(false) | Observation::PendingProvider => Ok(()),
        Observation::Known(true) => Err(ProgressFailure::Deadlock),
        Observation::Unproved => Err(ProgressFailure::Unproved),
        Observation::Supplied if scope == ProgressScope::OpenInterface => Ok(()),
        Observation::Supplied => Err(ProgressFailure::UnprovedExternal),
    }
}

pub(super) fn classify(
    memory: &Storage,
    location: &super::storage::Location,
    condition: SpinCondition,
    pivot_input: Option<terminal_psi::ValueDeclaration>,
    update: Option<CellUpdate<ScalarFact>>,
    requirements: &[Proposition],
) -> Observation {
    let assess =
        |stored: &Stored| observation(stored, condition, pivot_input, update, requirements);
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
    pivot_input: Option<terminal_psi::ValueDeclaration>,
    update: Option<CellUpdate<ScalarFact>>,
    requirements: &[Proposition],
) -> Observation {
    let mut all_block = true;
    let mut can_block = false;
    let visited = stored.visit_scalars(condition.comparison.pivot.scalar_type(), |value| {
        let cause = condition.failure(Some(value), pivot_input, update, requirements);
        all_block &= cause == Some(ProgressFailure::Deadlock);
        can_block |= cause.is_some();
    });
    let unresolved_can_block = condition
        .failure(None, pivot_input, update, requirements)
        .is_some();
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

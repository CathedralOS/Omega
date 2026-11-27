//! Replay every feasible returned alternative over the caller's actual cells.
use super::super::storage::Arrivals;
use super::{
    Bindings, Effect, NormalEffects, ProgressFailure, ProgressScope, NormalReturn, ScalarFact,
    Storage, Stored, Subject,
};
use semantic_vocabulary::Proposition;
use terminal_psi::StructuralArgument;

impl NormalEffects {
    pub(in crate::synchronization::private_cells) fn apply(
        &self,
        memory: &mut Storage,
        bindings: &Bindings<'_>,
        actuals: &[StructuralArgument],
        scalar: impl Fn(usize) -> Option<ScalarFact>,
        requirements: &[Proposition],
        scope: ProgressScope,
    ) -> Option<Result<(), ProgressFailure>> {
        let entry = memory.requirements(requirements);
        let possible = |path: &NormalReturn| {
            let mut inputs = Vec::new();
            for condition in &path.when {
                if let Some(answer) = condition.selector.answer(&scalar, &entry) {
                    if answer != condition.answer {
                        return false;
                    }
                } else if let Some(key) = condition.selector.actual_inputs(&scalar) {
                    if inputs
                        .iter()
                        .any(|&(known, answer)| known == key && answer != condition.answer)
                    {
                        return false;
                    }
                    inputs.push((key, condition.answer));
                }
            }
            true
        };
        let mut paths = self.paths.iter().filter(|path| possible(path));
        let first = paths.next()?;
        let Some(second) = paths.next() else {
            return first.apply(memory, bindings, actuals, &scalar, requirements, scope);
        };
        let mut joined: Option<Storage> = None;
        let mut failure = None;
        let mut opaque = false;
        for path in [first, second].into_iter().chain(paths) {
            let mut branch = memory.clone();
            match path.apply(&mut branch, bindings, actuals, &scalar, requirements, scope) {
                Some(Ok(())) => {
                    if let Some(previous) = &mut joined {
                        let before = previous.clone();
                        previous.join(&branch, Arrivals::new(&before, &branch));
                    } else {
                        joined = Some(branch);
                    }
                }
                Some(Err(cause)) => {
                    if failure.is_none() || cause != ProgressFailure::Deadlock {
                        failure = Some(cause);
                    }
                }
                None => opaque = true,
            }
        }
        if let Some(cause) = failure {
            let cause = if cause == ProgressFailure::Deadlock && (joined.is_some() || opaque) {
                ProgressFailure::Unproved
            } else {
                cause
            };
            return Some(Err(cause));
        }
        if opaque {
            return None;
        }
        *memory = joined?;
        Some(Ok(()))
    }
}

impl NormalReturn {
    fn apply(
        &self,
        memory: &mut Storage,
        bindings: &Bindings<'_>,
        actuals: &[StructuralArgument],
        scalar: &impl Fn(usize) -> Option<ScalarFact>,
        requirements: &[Proposition],
        scope: ProgressScope,
    ) -> Option<Result<(), ProgressFailure>> {
        for decision in &self.when {
            if let Some(condition) = decision.selector.actual_inputs(scalar) {
                memory.assume(condition.selected(decision.answer));
            }
        }
        let locate = |memory: &Storage, subject: &Subject| {
            let actual = actuals.get(subject.parameter)?;
            if actual.access == terminal_psi::StructuralAccess::Owned {
                return None;
            }
            let root = memory.locate(bindings, actual.place, &actual.path)?;
            memory.project(root, &subject.fixed_path()?)
        };
        for decision in &self.when {
            if let Some(test) = decision.selector.actual_inputs(scalar) {
                memory.assume(test.selected(decision.answer));
            }
        }
        for event in &self.events {
            match event {
                Effect::Acquire(acquisition) => {
                    let Some(place) = locate(memory, &acquisition.subject) else {
                        return Some(Err(ProgressFailure::UnprovedImport));
                    };
                    let observed = super::super::wait::classify(
                        memory,
                        &place,
                        acquisition.blocked,
                        None,
                        Some(acquisition.update),
                        requirements,
                    );
                    if let Err(cause) = super::super::wait::require_progress(observed, scope) {
                        return Some(Err(cause));
                    }
                    memory.set(
                        &place,
                        Stored::Scalar(ScalarFact::Literal(acquisition.held)),
                    );
                }
                Effect::Write(write) => {
                    let Some(place) = locate(memory, &write.subject) else {
                        // The caller still owes outstanding acquisitions. A write alone has no
                        // progress premise and can instead obscure a later wait.
                        return self
                            .events
                            .iter()
                            .any(|e| matches!(e, Effect::Acquire(_)))
                            .then_some(Err(ProgressFailure::UnprovedImport));
                    };
                    let value = write.value.binding(scalar);
                    memory.set(&place, Stored::from(value));
                }
            }
        }
        Some(Ok(()))
    }
}

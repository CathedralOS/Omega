//! Replay every feasible returned alternative over the caller's actual cells.
use super::super::storage::Arrivals;
use super::{
    Bindings, Effect, NormalEffects, NormalReturn, ProgressFailure, ProgressScope, ReturnedScalar,
    ScalarFact, Storage, Stored, Subject,
};
use crate::synchronization::progress::comparison::Evidence;
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
    ) -> Option<Result<Option<ScalarFact>, ProgressFailure>> {
        let entry = Evidence::new(requirements, &memory.conditions);
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
        let mut returned = None;
        let mut failure = None;
        let mut opaque = false;
        for path in [first, second].into_iter().chain(paths) {
            let mut branch = memory.clone();
            match path.apply(&mut branch, bindings, actuals, &scalar, requirements, scope) {
                Some(Ok(result)) => {
                    if let Some(previous) = &mut joined {
                        if returned != result {
                            returned = None;
                        }
                        let before = previous.clone();
                        previous.join(&branch, Arrivals::new(&before, &branch));
                    } else {
                        returned = result;
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
        Some(Ok(returned))
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
    ) -> Option<Result<Option<ScalarFact>, ProgressFailure>> {
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
        let mut ordinal = 0;
        let mut observed_result = None;
        for event in &self.events {
            match event {
                Effect::Observe {
                    subject,
                    scalar_type,
                    update,
                } => {
                    let Some(place) = locate(memory, subject) else {
                        return self
                            .events
                            .iter()
                            .any(|event| matches!(event, Effect::Acquire(_)))
                            .then_some(Err(ProgressFailure::UnprovedImport));
                    };
                    let prior = memory.get(&place);
                    if self.scalar_result
                        == Some(ReturnedScalar::Observation {
                            ordinal,
                            scalar_type: *scalar_type,
                        })
                        && !place.includes_shared()
                    {
                        observed_result = prior.scalar(*scalar_type);
                    }
                    ordinal += 1;
                    if let Some(next) = prior
                        .after_observation(*scalar_type, *update, |operand| operand.binding(scalar))
                    {
                        memory.set(&place, next);
                    }
                }
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
        Some(Ok(match self.scalar_result {
            Some(ReturnedScalar::Operand(operand)) => operand.binding(scalar),
            Some(ReturnedScalar::Observation { .. }) => observed_result,
            None => None,
        }))
    }
}

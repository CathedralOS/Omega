//! A selected provider can acquire locks, write through a borrowed argument, or
//! run a destructor that releases locks. Translate the proven normal-return
//! effects in execution order before the caller makes its next observation.

mod guards;
mod sequence;

use super::activation::Bindings;
use super::storage::{Storage, Stored};
use crate::entry_values::TerminalScalarValue as Scalar;
use crate::synchronization::progress::{ProgressFailure, ProgressScope, ScalarFact};
use crate::synchronization::spin::{CellUpdate, Operand, SpinCondition, Subject};
pub(in crate::synchronization) use sequence::normal_effects;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Acquisition {
    subject: Subject,
    held: Scalar,
    blocked: SpinCondition,
    update: CellUpdate<ScalarFact>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AtomicWrite {
    subject: Subject,
    value: Operand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Effect {
    Acquire(Acquisition),
    Write(AtomicWrite),
}

/// Effects derived from the exact selected body, ordered before its normal return.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalEffects(Vec<Effect>);

impl NormalEffects {
    pub fn requires_progress(&self) -> bool {
        self.0
            .iter()
            .any(|event| matches!(event, Effect::Acquire(_)))
    }

    pub(in crate::synchronization) fn map_operands(
        &mut self,
        mut apply: impl FnMut(Operand) -> Option<Operand>,
    ) -> Option<()> {
        for event in &mut self.0 {
            if let Effect::Write(write) = event {
                write.value = apply(write.value)?;
            }
        }
        Some(())
    }

    pub(in crate::synchronization) fn map_subjects(
        &mut self,
        mut apply: impl FnMut(&mut Subject) -> Option<()>,
    ) -> Option<()> {
        for event in &mut self.0 {
            apply(match event {
                Effect::Acquire(acquisition) => &mut acquisition.subject,
                Effect::Write(write) => &mut write.subject,
            })?;
        }
        Some(())
    }

    pub(super) fn apply(
        &self,
        memory: &mut Storage,
        bindings: &Bindings<'_>,
        actuals: &[terminal_psi::StructuralArgument],
        scalar: impl Fn(usize) -> Option<ScalarFact>,
        requirements: &[semantic_vocabulary::Proposition],
        scope: ProgressScope,
    ) -> Option<Result<(), ProgressFailure>> {
        let locate = |memory: &Storage, subject: &Subject| {
            let actual = actuals.get(subject.parameter)?;
            if actual.access == terminal_psi::StructuralAccess::Owned {
                return None;
            }
            let root = memory.locate(bindings, actual.place, &actual.path)?;
            memory.project(root, &subject.fixed_path()?)
        };
        for event in &self.0 {
            match event {
                Effect::Acquire(acquisition) => {
                    let Some(place) = locate(memory, &acquisition.subject) else {
                        return Some(Err(ProgressFailure::UnprovedImport));
                    };
                    let observed = super::wait::classify(
                        memory,
                        &place,
                        acquisition.blocked,
                        None,
                        Some(acquisition.update),
                        requirements,
                    );
                    if let Err(cause) = super::wait::require_progress(observed, scope) {
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
                            .requires_progress()
                            .then_some(Err(ProgressFailure::UnprovedImport));
                    };
                    let value = match write.value {
                        Operand::Literal(value) => Some(ScalarFact::Literal(value)),
                        Operand::Parameter {
                            position,
                            scalar_type,
                        } => scalar(position).filter(|fact| fact.ty() == scalar_type),
                    };
                    memory.set(&place, Stored::from(value));
                }
            }
        }
        Some(Ok(()))
    }
}

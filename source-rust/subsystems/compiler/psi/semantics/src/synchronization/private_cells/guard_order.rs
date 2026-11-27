//! Derive what a successful guard constructor acquires and what consuming its
//! result actually writes. The cleanup starts with the returned record alone.
//! A reference to the other lock therefore cannot release the acquired one.

mod body;
use body::GuardEvent;
pub use body::GuardOrder;
pub(in crate::synchronization) use body::guard_order;

use super::activation::{Bindings, Destination, Frame};
use super::storage::{Location, Storage, Stored};
use super::successor;
use crate::entry_values::TerminalScalarValue as Scalar;
use crate::synchronization::progress::{ProgressFailure, ProgressScope, ScalarFact, entry_scalars};
use crate::synchronization::selected::{SelectedCall, Summaries};
use crate::synchronization::spin::{CellUpdate, SpinCondition};
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{
    AtomicAccessEvent, OperationKind, StructuralAccess, TerminalAffineCleanupAction,
    TerminalMachine, TerminalModule, Terminator,
};

use crate::synchronization::spin::Subject;

#[derive(Clone, Debug, PartialEq, Eq)]
struct GuardAcquisition {
    subject: Subject,
    held: Scalar,
    blocked: SpinCondition,
    update: CellUpdate<ScalarFact>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GuardCleanupWrite {
    subject: Subject,
    value: Scalar,
}

/// Acquisitions occur in order and remain held on the successful return.
/// Cleanup writes occur only when the selected destructor consumes that result.
/// Keep the writes in order: different symbolic parameters may alias after
/// substitution, and a later write can restore the blocking word.
#[derive(Clone, Debug, PartialEq, Eq)]
struct OwnedGuardLocks {
    acquisitions: Vec<GuardAcquisition>,
    cleanup: MachineId,
    cleanup_writes: Vec<GuardCleanupWrite>,
}

struct Analysis<'m, 'b, B> {
    module: &'m TerminalModule,
    boundary: &'b B,
    spins: Summaries,
    storage: Storage,
    roots: Vec<Location>,
    acquired: Vec<GuardAcquisition>,
    writes: Vec<GuardCleanupWrite>,
}

impl<'m, B: Fn(BoundaryMachineId) -> Option<SelectedCall>> Analysis<'m, '_, B> {
    fn subject(&self, location: &Location) -> Option<Subject> {
        self.roots.iter().enumerate().find_map(|(parameter, root)| {
            let path = root.suffix(location)?;
            Subject::from_fixed(parameter, path)
        })
    }

    fn acquire(&mut self, frame: &Frame<'_>, op: &terminal_psi::Operation) -> Option<()> {
        let call = op.declared_invocation()?;
        if !matches!(op.result, terminal_psi::OperationResult::Unit)
            || !call.crash_continuations.is_empty()
        {
            return None;
        }
        let wait = match SelectedCall::resolve(call.callee, self.boundary)? {
            SelectedCall::Local(id) => self.spins.get(self.module, id, self.boundary)?,
            SelectedCall::ImportedWait(wait) => wait,
            _ => return None,
        };
        let wait = wait.single_cell()?;
        let mut enabled = wait.while_value;
        enabled.positive = !enabled.positive;
        let update = wait.update.try_map(|operand| operand.literal())?;
        let held = match update {
            CellUpdate::Replace(value) => value,
            CellUpdate::CompareExchange {
                expected,
                replacement,
            } if enabled.sole_blocked_value() == Some(expected) => replacement,
            _ => return None,
        };
        if wait.while_value.matches(held) != Some(true) {
            return None;
        }
        let actual = call.structural_arguments.get(wait.subject.parameter)?;
        if actual.access != StructuralAccess::SharedBorrow {
            return None;
        }
        let root = self
            .storage
            .locate(&frame.bindings, actual.place, &actual.path)?;
        let target = self.storage.project(root, &wait.subject.fixed_path()?)?;
        let subject = self.subject(&target)?;
        self.acquired.push(GuardAcquisition {
            subject,
            held,
            blocked: wait.while_value,
            update: update.try_map(|value| Some(ScalarFact::Literal(value)))?,
        });
        self.storage
            .set(&target, Stored::Scalar(ScalarFact::Literal(held)));
        Some(())
    }

    fn body(
        &mut self,
        machine: &'m TerminalMachine,
        bindings: Bindings<'m>,
        cleanup: bool,
    ) -> Option<Frame<'m>> {
        let mut frame = Frame::enter(machine, bindings, Destination::Unit)?;
        loop {
            for op in &frame.block.operations {
                if op.declared_invocation().is_some() {
                    if cleanup {
                        return None;
                    }
                    self.acquire(&frame, op)?;
                    continue;
                }
                match &op.kind {
                    OperationKind::AtomicAccess {
                        place,
                        path,
                        event: AtomicAccessEvent::Store { value, .. },
                        resident_obligation: None,
                    } if cleanup => {
                        let target = self.storage.locate(&frame.bindings, *place, path)?;
                        let subject = self.subject(&target)?;
                        let ScalarFact::Literal(value) = *frame.bindings.values.get(value)? else {
                            return None;
                        };
                        self.writes.push(GuardCleanupWrite { subject, value });
                    }
                    OperationKind::AtomicAccess { .. } => return None,
                    OperationKind::StoreStructuralField {
                        destination, path, ..
                    }
                    | OperationKind::StructuralScalarFieldStore {
                        destination, path, ..
                    } => {
                        let target = self.storage.locate(&frame.bindings, *destination, path)?;
                        if self.subject(&target).is_some() {
                            return None;
                        }
                    }
                    _ => {}
                }
                frame.step(self.module, op, &mut self.storage)?;
            }
            let successor::Selection::Ready(actions) =
                successor::prepare(self.module, &mut frame, &mut self.storage)?
            else {
                return None;
            };
            if actions
                .iter()
                .any(|action| matches!(action, TerminalAffineCleanupAction::InvokeOwned(_)))
            {
                return None;
            }
            if frame.arrival.is_none() {
                return Some(frame);
            }
            frame.arrive()?;
        }
    }
}

fn guard_locks(
    module: &TerminalModule,
    constructor: MachineId,
    boundary: impl Fn(BoundaryMachineId) -> Option<SelectedCall>,
) -> Option<OwnedGuardLocks> {
    let machine = module.machine(constructor)?;
    let result = machine.result.structural()?;
    let cleanup = module
        .structural_type(result.structural_type)?
        .nominal_cleanup
        .as_ref()?
        .cleanup_machine;
    let destructor = module.machine(cleanup)?;
    let [guard] = destructor.structural_parameters.as_slice() else {
        return None;
    };
    if !destructor.parameters.is_empty()
        || guard.access != StructuralAccess::Owned
        || guard.structural_type != result.structural_type
    {
        return None;
    }

    let mut analysis = Analysis {
        module,
        boundary: &boundary,
        spins: Summaries::default(),
        storage: Storage::default(),
        roots: Vec::with_capacity(machine.structural_parameters.len()),
        acquired: Vec::new(),
        writes: Vec::new(),
    };
    let mut bindings = Bindings {
        values: entry_scalars(machine),
        ..Bindings::new(module, machine)
    };
    for formal in &machine.structural_parameters {
        if !matches!(formal.access, terminal_psi::reading_borrow_pair_pattern!()) {
            return None;
        }
        let root = analysis.storage.allocate(Stored::Unknown);
        bindings.places.insert(formal.place, root.clone());
        analysis.roots.push(root);
    }
    let finished = analysis.body(machine, bindings, false)?;
    let Terminator::ReturnStructural { source, .. } = &finished.block.terminator else {
        return None;
    };
    if analysis.acquired.is_empty() {
        return None;
    }
    let returned = finished.bindings.places.get(source)?.clone();
    let mut bindings = Bindings::new(module, destructor);
    bindings.places.insert(guard.place, returned);
    let finished = analysis.body(destructor, bindings, true)?;
    if !matches!(finished.block.terminator, Terminator::ReturnUnit { .. }) {
        return None;
    }
    Some(OwnedGuardLocks {
        acquisitions: analysis.acquired,
        cleanup,
        cleanup_writes: analysis.writes,
    })
}

impl GuardOrder {
    pub(super) fn apply(
        &self,
        memory: &mut Storage,
        bindings: &Bindings<'_>,
        actuals: &[terminal_psi::StructuralArgument],
        requirements: &[semantic_vocabulary::Proposition],
        scope: ProgressScope,
    ) -> Result<(), ProgressFailure> {
        let locate = |memory: &Storage, subject: &Subject| {
            let actual = actuals.get(subject.parameter)?;
            if !matches!(actual.access, terminal_psi::reading_borrow_pair_pattern!()) {
                return None;
            }
            let root = memory.locate(bindings, actual.place, &actual.path)?;
            memory.project(root, &subject.fixed_path()?)
        };
        for event in &self.events {
            match event {
                GuardEvent::Acquired { acquisitions, .. } => {
                    for acquisition in acquisitions {
                        let place = locate(memory, &acquisition.subject)
                            .ok_or(ProgressFailure::Unproved)?;
                        let observation = super::wait::classify(
                            memory,
                            &place,
                            acquisition.blocked,
                            None,
                            Some(acquisition.update),
                            requirements,
                        );
                        super::wait::require_progress(observation, scope)?;
                        memory.set(
                            &place,
                            Stored::Scalar(ScalarFact::Literal(acquisition.held)),
                        );
                    }
                }
                GuardEvent::Consumed { writes, .. } => {
                    for write in writes {
                        let place =
                            locate(memory, &write.subject).ok_or(ProgressFailure::Unproved)?;
                        memory.set(&place, Stored::Scalar(ScalarFact::Literal(write.value)));
                    }
                }
            }
        }
        Ok(())
    }
}

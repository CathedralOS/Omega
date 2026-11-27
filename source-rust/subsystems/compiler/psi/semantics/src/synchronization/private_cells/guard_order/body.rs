//! Preserve acquisition order while each owned result is live. A move through
//! a local record carries the guard to its cleanup, just as a borrowed field
//! continues to designate its original storage.

use super::{
    Bindings, Destination, Frame, GuardAcquisition, GuardCleanupWrite, Location, OwnedGuardLocks,
    SelectedCall, Storage, Stored, Subject, entry_scalars, guard_locks, successor,
};
use crate::synchronization::facts::Facts;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{
    OperationKind, StructuralArgument, TargetMachineCallable, TerminalAffineCleanupAction,
    TerminalModule, Terminator,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum GuardEvent {
    Acquired {
        guard: usize,
        acquisitions: Vec<GuardAcquisition>,
    },
    Consumed {
        guard: usize,
        writes: Vec<GuardCleanupWrite>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardOrder {
    pub(super) events: Vec<GuardEvent>,
}

pub(in crate::synchronization) fn guard_order(
    module: &TerminalModule,
    entry: MachineId,
    boundary: impl Fn(BoundaryMachineId) -> Option<SelectedCall>,
) -> Option<GuardOrder> {
    let mut wrappers = Vec::new();
    let mut seen = Facts::default();
    let mut current = entry;
    let machine = loop {
        if seen.insert(current, ()).is_some() {
            return None;
        }
        let machine = module.machine(current)?;
        let Some(wrapper) = crate::synchronization::spin::forwarded_wait(module, machine) else {
            break machine;
        };
        let TargetMachineCallable::Machine(callee) = wrapper.callee else {
            return None;
        };
        wrappers.push(wrapper);
        current = callee;
    };
    let mut order = order_in_body(module, machine, boundary)?;
    for wrapper in wrappers.iter().rev() {
        order.map_subjects(|subject| wrapper.apply_subject(subject))?;
    }
    Some(order)
}

impl GuardOrder {
    pub(in crate::synchronization) fn map_subjects(
        &mut self,
        mut apply: impl FnMut(&mut Subject) -> Option<()>,
    ) -> Option<()> {
        for event in &mut self.events {
            match event {
                GuardEvent::Acquired { acquisitions, .. } => {
                    for step in acquisitions {
                        apply(&mut step.subject)?;
                    }
                }
                GuardEvent::Consumed { writes, .. } => {
                    for write in writes {
                        apply(&mut write.subject)?;
                    }
                }
            }
        }
        Some(())
    }
}

fn order_in_body(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    boundary: impl Fn(BoundaryMachineId) -> Option<SelectedCall>,
) -> Option<GuardOrder> {
    let mut memory = Storage::default();
    let mut roots = Vec::with_capacity(machine.structural_parameters.len());
    let mut bindings = Bindings {
        values: entry_scalars(machine),
        ..Bindings::new(module, machine)
    };
    for (parameter, formal) in machine.structural_parameters.iter().enumerate() {
        // Owned context tokens need no symbolic lock identity. All retained
        // lock subjects must resolve to storage borrowed from this entry.
        let root = memory.allocate(Stored::Unknown);
        if matches!(formal.access, terminal_psi::reading_borrow_pair_pattern!()) {
            roots.push((parameter, root.clone()));
        }
        bindings.places.insert(formal.place, root);
    }
    let mut frame = Frame::enter(machine, bindings, Destination::Unit)?;
    let mut events = Vec::new();
    let mut guards = Vec::new();
    let mut constructors = Facts::<MachineId, Option<OwnedGuardLocks>>::default();
    loop {
        for op in &frame.block.operations {
            if let Some(call) = op.declared_invocation() {
                if !call.crash_continuations.is_empty() {
                    return None;
                }
                let TargetMachineCallable::Machine(id) = call.callee else {
                    return None;
                };
                if constructors.get(&id).is_none() {
                    constructors.insert(id, guard_locks(module, id, &boundary));
                }
                let summary = constructors.get(&id)?.as_ref()?;
                let project = |subject: &Subject| {
                    substitute(
                        subject,
                        call.structural_arguments,
                        &frame.bindings,
                        &memory,
                        &roots,
                    )
                };
                let acquisitions = summary
                    .acquisitions
                    .iter()
                    .map(|source| {
                        let mut target = source.clone();
                        target.subject = project(&source.subject)?;
                        Some(target)
                    })
                    .collect::<Option<_>>()?;
                let writes = summary
                    .cleanup_writes
                    .iter()
                    .map(|source| {
                        Some(GuardCleanupWrite {
                            subject: project(&source.subject)?,
                            value: source.value,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let guard = guards.len();
                guards.push(Some((summary.cleanup, writes)));
                events.push(GuardEvent::Acquired {
                    guard,
                    acquisitions,
                });
                frame.bindings.places.insert(
                    op.result.structural()?.place,
                    memory.allocate(Stored::Guard(guard)),
                );
            } else {
                match &op.kind {
                    OperationKind::AtomicAccess { .. } => return None,
                    OperationKind::StoreStructuralField {
                        destination, path, ..
                    }
                    | OperationKind::StructuralScalarFieldStore {
                        destination, path, ..
                    } => {
                        let target = memory.locate(&frame.bindings, *destination, path)?;
                        if roots.iter().any(|(_, root)| root.suffix(&target).is_some()) {
                            return None;
                        }
                    }
                    _ => {}
                }
                frame.step(module, op, &mut memory)?;
            }
        }
        let successor::Selection::Ready(actions) =
            successor::prepare(module, &mut frame, &mut memory)?
        else {
            return None;
        };
        for action in actions {
            let TerminalAffineCleanupAction::InvokeOwned(invocation) = action else {
                continue;
            };
            let actual = &invocation.subject;
            let place = memory.locate(&frame.bindings, actual.place, &actual.path)?;
            let Stored::Guard(guard) = memory.get(&place) else {
                return None;
            };
            let (cleanup, writes) = guards.get_mut(guard)?.take()?;
            if cleanup != invocation.cleanup_machine {
                return None;
            }
            events.push(GuardEvent::Consumed { guard, writes });
        }
        if frame.arrival.is_none() {
            return (!events.is_empty()
                && matches!(frame.block.terminator, Terminator::ReturnUnit { .. }))
            .then_some(GuardOrder { events });
        }
        frame.arrive()?;
    }
}

fn substitute(
    subject: &Subject,
    actuals: &[StructuralArgument],
    bindings: &Bindings<'_>,
    memory: &Storage,
    roots: &[(usize, Location)],
) -> Option<Subject> {
    let actual = actuals.get(subject.parameter)?;
    let root = memory.locate(bindings, actual.place, &actual.path)?;
    let target = memory.project(root, &subject.fixed_path()?)?;
    roots
        .iter()
        .find_map(|(parameter, root)| Subject::from_fixed(*parameter, root.suffix(&target)?))
}

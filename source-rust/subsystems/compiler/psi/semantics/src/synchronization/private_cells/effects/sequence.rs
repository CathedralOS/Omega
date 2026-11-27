//! Interpret one verified unit-returning path symbolically. An owned guard
//! contributes its cleanup writes only when that exact value is consumed.
use super::super::activation::{Bindings, Destination, Frame};
use super::super::storage::{Location, Storage, Stored};
use super::super::successor;
use super::guards::{OwnedGuardLocks, guard_locks};
use super::{AtomicWrite, Effect, NormalEffects, Operand, ScalarFact, Subject};
use crate::synchronization::facts::Facts;
use crate::synchronization::progress::entry_scalars;
use crate::synchronization::selected::SelectedCall;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{
    AtomicAccessEvent, OperationKind, StructuralArgument, TargetMachineCallable,
    TerminalAffineCleanupAction, TerminalModule, Terminator,
};

pub(in crate::synchronization) fn normal_effects(
    module: &TerminalModule,
    entry: MachineId,
    boundary: impl Fn(BoundaryMachineId) -> Option<SelectedCall>,
) -> Option<NormalEffects> {
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
    let mut effects = effects_in_body(module, machine, boundary)?;
    for wrapper in wrappers.iter().rev() {
        effects.map_operands(|operand| wrapper.scalar(operand))?;
        effects.map_subjects(|subject| wrapper.apply_subject(subject))?;
    }
    Some(effects)
}

fn effects_in_body(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    boundary: impl Fn(BoundaryMachineId) -> Option<SelectedCall>,
) -> Option<NormalEffects> {
    let mut memory = Storage::default();
    let mut roots = Vec::with_capacity(machine.structural_parameters.len());
    let mut bindings = Bindings {
        values: entry_scalars(machine),
        ..Bindings::new(module, machine)
    };
    for (parameter, formal) in machine.structural_parameters.iter().enumerate() {
        // An effect is visible to the caller only through a borrowed argument.
        let root = memory.allocate(Stored::Unknown);
        if formal.access != terminal_psi::StructuralAccess::Owned {
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
                    .collect::<Option<Vec<_>>>()?;
                let writes = summary
                    .cleanup_writes
                    .iter()
                    .map(|source| {
                        Some(AtomicWrite {
                            subject: project(&source.subject)?,
                            value: source.value,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let guard = guards.len();
                guards.push(Some((summary.cleanup, writes)));
                events.extend(acquisitions.into_iter().map(Effect::Acquire));
                frame.bindings.places.insert(
                    op.result.structural()?.place,
                    memory.allocate(Stored::Guard(guard)),
                );
            } else {
                match &op.kind {
                    OperationKind::AtomicAccess {
                        place,
                        path,
                        event: AtomicAccessEvent::Store { value, .. },
                        resident_obligation: None,
                    } => {
                        let target = memory.locate(&frame.bindings, *place, path)?;
                        let subject = subject_of(&roots, &target)?;
                        let value = match *frame.bindings.values.get(value)? {
                            ScalarFact::Literal(value) => Operand::Literal(value),
                            ScalarFact::EntryInput(formal) => Operand::Parameter {
                                position: machine
                                    .parameters
                                    .iter()
                                    .position(|entry| *entry == formal)?,
                                scalar_type: formal.scalar_type,
                            },
                        };
                        events.push(Effect::Write(AtomicWrite { subject, value }));
                    }
                    OperationKind::AtomicAccess { .. } => return None,
                    OperationKind::MoveStructuralValue { source }
                    | OperationKind::DecomposeStructuralValue { subject: source }
                        if roots.iter().any(|(_, root)| {
                            frame
                                .bindings
                                .places
                                .get(&source.place)
                                .is_some_and(|home| root.suffix(home).is_some())
                        }) =>
                    {
                        return None;
                    }
                    OperationKind::MoveStructuralField {
                        source: destination,
                        path,
                        ..
                    }
                    | OperationKind::StoreStructuralField {
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
            events.extend(writes.into_iter().map(Effect::Write));
        }
        if frame.arrival.is_none() {
            let empty_return = machine.blocks.len() == 1 && machine.operations().next().is_none();
            return ((!events.is_empty() || empty_return)
                && matches!(frame.block.terminator, Terminator::ReturnUnit { .. }))
            .then_some(NormalEffects(events));
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
    subject_of(roots, &target)
}
fn subject_of(roots: &[(usize, Location)], target: &Location) -> Option<Subject> {
    roots.iter().find_map(|(parameter, root)| Subject::from_fixed(*parameter,root.suffix(target)?))
}

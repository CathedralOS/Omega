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

pub(super) fn effects_in_body(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    boundary: &impl Fn(BoundaryMachineId) -> Option<SelectedCall>,
    completed: &Facts<MachineId, Option<NormalEffects>>,
    program_size: usize,
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
    let mut effects = NormalEffects::default();
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
                let project = |subject: &Subject| {
                    substitute(
                        subject,
                        call.structural_arguments,
                        &frame.bindings,
                        &memory,
                        &roots,
                    )
                };
                if matches!(op.result, terminal_psi::OperationResult::Unit) {
                    let mut nested = completed.get(&id)?.as_ref()?.clone();
                    nested.map_subjects(|subject| {
                        *subject = project(subject)?;
                        Some(())
                    })?;
                    nested.map_operands(|value| {
                        let Operand::Parameter {
                            position,
                            scalar_type,
                        } = value
                        else {
                            return Some(value);
                        };
                        let actual = call.arguments.get(position)?;
                        let operand = operand(machine, *frame.bindings.values.get(actual)?)?;
                        (operand.scalar_type() == scalar_type).then_some(operand)
                    })?;
                    effects.append_call(nested, program_size)?;
                    continue;
                }
                if constructors.get(&id).is_none() {
                    constructors.insert(id, guard_locks(module, id, boundary));
                }
                let summary = constructors.get(&id)?.as_ref()?;
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
                for acquisition in acquisitions {
                    effects.0.push(Effect::Acquire(acquisition));
                }
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
                        let value = operand(machine, *frame.bindings.values.get(value)?)?;
                        effects
                            .0
                            .push(Effect::Write(AtomicWrite { subject, value }));
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
            for write in writes {
                effects.0.push(Effect::Write(write));
            }
        }
        if frame.arrival.is_none() {
            if !matches!(frame.block.terminator, Terminator::ReturnUnit { .. }) {
                return None;
            }
            effects.compact_stores();
            return Some(effects);
        }
        frame.arrive()?;
    }
}

fn operand(machine: &terminal_psi::TerminalMachine, fact: ScalarFact) -> Option<Operand> {
    Some(match fact {
        ScalarFact::Literal(value) => Operand::Literal(value),
        ScalarFact::EntryInput(formal) => Operand::Parameter {
            position: machine
                .parameters
                .iter()
                .position(|parameter| *parameter == formal)?,
            scalar_type: formal.scalar_type,
        },
    })
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
    roots
        .iter()
        .find_map(|(parameter, root)| Subject::from_fixed(*parameter, root.suffix(target)?))
}

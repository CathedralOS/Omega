//! Instantiate derived wait predicates on fresh, unescaped storage. Borrow
//! arguments identify the same allocation across checked calls and jumps.

mod activation;
mod aggregate;
mod control;
mod opaque_calls;
mod storage;
mod successor;
mod wait;

use super::progress::{PrivateWaitFailure, ProgressFailure, ProgressScope, entry_scalars};
use super::selected::{SelectedCall, Summaries};
use super::spin::spin_observation;
use activation::{Bindings, Destination, Frame, ScalarArguments};
use control::{Execution, Frontier};
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use storage::{Location, Storage};
use terminal_psi::{
    StructuralAccess, TerminalAffineCleanupAction, TerminalModule,
    Terminator,
};

pub fn private_wait_failure(
    module: &TerminalModule,
    entry: MachineId,
    scope: ProgressScope,
    boundary: impl Fn(BoundaryMachineId) -> Option<SelectedCall>,
) -> Option<PrivateWaitFailure> {
    if !module.types.structural_types.iter().any(|ty| {
        matches!(ty.shape, terminal_psi::StructuralTypeShape::AtomicCell(_))
            || ty
                .shape
                .fields()
                .any(|field| field.field_type.atomic_cell_type().is_some())
    }) {
        return None;
    }
    let selected_entry = module.machine(entry)?;
    let mut root_bindings = Bindings {
        values: entry_scalars(selected_entry),
        ..Default::default()
    };
    if crate::inhabitation::predicates_are_false_with(&selected_entry.requires, |value| {
        root_bindings.values.get(&value)?.known_integer()
    }) {
        return None;
    }
    let mut memory = Storage::default();
    for parameter in &selected_entry.structural_parameters {
        let home = if parameter.access == StructuralAccess::SharedBorrow {
            Location::input(parameter.place)
        } else {
            memory.input()
        };
        root_bindings.places.insert(parameter.place, home);
    }
    if let Some((operation, wait)) = spin_observation(module, selected_entry) {
        let arguments: Vec<_> = selected_entry
            .structural_parameters
            .iter()
            .map(|formal| terminal_psi::StructuralArgument::whole(formal.place, formal.access))
            .collect();
        let cause = wait::apply(
            &wait,
            &root_bindings,
            &mut memory,
            &arguments,
            &selected_entry.requires,
            scope,
        )
        .err()?;
        return Some(PrivateWaitFailure {
            machine: entry,
            operation,
            cause,
        });
    }
    let entry = Frame::enter(selected_entry, root_bindings, Destination::Unit)?;
    let mut paths = Frontier::default();
    paths.queue(Execution {
        stack: vec![entry],
        memory,
        coverage: Default::default(),
    });
    let mut summaries = Summaries::default();
    let mut unproved = None;
    while let Some(mut path) = paths.next() {
        match advance(
            module,
            &boundary,
            &mut summaries,
            &mut path,
            &selected_entry.requires,
            scope,
        ) {
            Some(Advance::Continue) => paths.queue(path),
            Some(Advance::Fork(arms)) => path.fork(&arms, |arm| paths.queue(arm)),
            Some(Advance::Blocked(failure)) if failure.cause == ProgressFailure::Deadlock => {
                return Some(failure);
            }
            Some(Advance::Blocked(failure)) => {
                unproved.get_or_insert(failure);
            }
            None => {}
        }
    }
    unproved
}

enum Advance {
    Continue,
    Fork(std::borrow::Cow<'static, [usize]>),
    Blocked(PrivateWaitFailure),
}

fn advance<'m>(
    module: &'m TerminalModule,
    boundary: &impl Fn(BoundaryMachineId) -> Option<SelectedCall>,
    summaries: &mut Summaries,
    path: &mut Execution<'m>,
    requirements: &[semantic_vocabulary::Proposition],
    scope: ProgressScope,
) -> Option<Advance> {
    let certain = path.certain();
    let memory = &mut path.memory;
    let stack = &mut path.stack;
    while let Some(frame) = stack.last_mut() {
        if let Some(op) = frame.block.operations.get(frame.position) {
            frame.position += 1;
            let Some(call) = op.declared_invocation() else {
                frame.step(module, op, memory)?;
                continue;
            };
            let Some(selected) = SelectedCall::resolve(call.callee, boundary) else {
                let terminal_psi::TargetMachineCallable::Boundary(slot) = call.callee else { return None };
                if !opaque_calls::returns_without_storage(module, slot, op, &call)? {
                    path.coverage = control::Coverage::Partial;
                }
                return Some(Advance::Continue);
            };
            let spin = match selected {
                SelectedCall::Local(id) => {
                    if let Some(summary) = summaries.get(module, id, boundary) {
                        summary
                    } else {
                        let callee = module.machine(id)?;
                        let args = frame.bindings.arguments(
                            memory,
                            &callee.parameters,
                            &callee.structural_parameters,
                            ScalarArguments::Values(call.arguments),
                            call.structural_arguments,
                        )?;
                        let return_to = Destination::of(op);
                        if stack.iter().any(|active| active.machine.id == id) {
                            return None;
                        }
                        stack.push(Frame::enter(callee, args, return_to)?);
                        continue;
                    }
                }
                SelectedCall::ImportedWait(wait) => wait,
            };
            let progress = wait::apply(
                &spin,
                &frame.bindings,
                memory,
                call.structural_arguments,
                requirements,
                scope,
            );
            if let Err(mut cause) = progress {
                if !certain && cause == ProgressFailure::Deadlock {
                    cause = ProgressFailure::Unproved;
                }
                return Some(Advance::Blocked(PrivateWaitFailure {
                    machine: frame.machine.id,
                    operation: op.id,
                    cause,
                }));
            }
            continue;
        }
        let cleanup = match successor::prepare(module, frame, memory)? {
            successor::Selection::Fork(arms) => return Some(Advance::Fork(arms)),
            successor::Selection::Ready(actions) => actions,
        };
        if let Some(action) = cleanup.get(frame.cleanup_cursor) {
            frame.cleanup_cursor += 1;
            if let TerminalAffineCleanupAction::InvokeOwned(invocation) = action {
                let callee = module.machine(invocation.cleanup_machine)?;
                let args = frame.bindings.arguments(
                    memory,
                    &callee.parameters,
                    &callee.structural_parameters,
                    ScalarArguments::Values(&[]),
                    std::slice::from_ref(&invocation.subject),
                )?;
                if stack.iter().any(|active| active.machine.id == callee.id) {
                    return None;
                }
                stack.push(Frame::enter(callee, args, Destination::Unit)?);
            }
            continue;
        }
        if frame.arrival.is_some() {
            frame.arrive()?;
            return Some(Advance::Continue);
        }
        let frame = stack.pop()?;
        if let Some(caller) = stack.last_mut() {
            match (frame.return_to, &frame.block.terminator) {
                (Destination::Unit, Terminator::ReturnUnit { .. }) => {}
                (Destination::Scalar(dest), Terminator::Return { value, .. }) => {
                    if let Some(value) = frame.bindings.values.get(value) {
                        caller.bindings.values.insert(dest, *value);
                    }
                }
                (Destination::Structural(dest), Terminator::ReturnStructural { source, .. }) => {
                    let location = frame.bindings.places.get(source)?;
                    let returned = memory.allocate(memory.get(location));
                    caller.bindings.places.insert(dest, returned);
                }
                _ => return None,
            }
            return Some(Advance::Continue);
        }
    }
    None
}

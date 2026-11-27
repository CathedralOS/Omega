//! Instantiate derived wait predicates on fresh, unescaped storage. Borrow
//! arguments identify the same allocation across checked calls and jumps.

mod activation;
mod aggregate;
mod control;
mod storage;

use super::progress::{PrivateWaitFailure, ProgressFailure, ScalarFact, entry_scalars};
use super::selected::Summaries;
use super::spin::{CellUpdate, SpinWait};
use crate::entry_values::TerminalScalarValue as Scalar;
use activation::{Bindings, Destination, Frame, ScalarArguments};
use control::{Execution, Frontier};
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
use storage::Storage;
use terminal_psi::{
    StructuralAccess, TargetMachineCallable, TerminalAffineCleanupAction, TerminalModule,
    Terminator,
};

/// Find a call which can only retry on a permanently unchanged private cell.
/// The caller supplies summaries only for its exact admitted boundary bodies.
/// Follow returned records and their selected cleanup on the actual edge.
/// Stop upon unknown effects, control or aliasing and repeated ordinary blocks.
pub fn stuck_private_wait(
    module: &TerminalModule,
    entry: MachineId,
    boundary: impl Fn(BoundaryMachineId) -> Option<SpinWait>,
) -> Option<(MachineId, OperationId)> {
    let failure = private_wait_failure(module, entry, boundary)?;
    (failure.cause == ProgressFailure::Deadlock).then_some((failure.machine, failure.operation))
}

pub fn private_wait_failure(
    module: &TerminalModule,
    entry: MachineId,
    boundary: impl Fn(BoundaryMachineId) -> Option<SpinWait>,
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
    let root_bindings = Bindings {
        values: entry_scalars(selected_entry),
        ..Default::default()
    };
    if crate::inhabitation::predicates_are_false_with(&selected_entry.requires, |value| {
        root_bindings.values.get(&value)?.known_integer()
    }) {
        return None;
    }
    let entry = Frame::enter(selected_entry, root_bindings, Destination::Unit)?;
    let mut paths = Frontier::default();
    paths.queue(Execution {
        stack: vec![entry],
        memory: Storage::default(),
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
        ) {
            Some(Advance::Continue) => paths.queue(path),
            Some(Advance::Fork) => {
                let other = path.fork()?;
                paths.queue(other);
                paths.queue(path);
            }
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
    Fork,
    Blocked(PrivateWaitFailure),
}

fn advance<'m>(
    module: &'m TerminalModule,
    boundary: &impl Fn(BoundaryMachineId) -> Option<SpinWait>,
    summaries: &mut Summaries,
    path: &mut Execution<'m>,
    requirements: &[semantic_vocabulary::Proposition],
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
            let spin = match call.callee {
                TargetMachineCallable::Machine(id) => {
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
                TargetMachineCallable::Boundary(id) => boundary(id)?,
            };
            let actual = call.structural_arguments.get(spin.parameter)?;
            if actual.access != StructuralAccess::SharedBorrow {
                return None;
            }
            let base = memory.locate(&frame.bindings.places, actual.place, &actual.path)?;
            let location = memory.project(base, &spin.path)?;
            let mut all_block = true;
            let mut can_block = false;
            let mut observe = |initial| {
                let failure = spin.while_value.failure(initial, spin.update, requirements);
                all_block &= failure == Some(ProgressFailure::Deadlock);
                can_block |= failure.is_some();
            };
            if memory
                .get(&location)
                .visit_scalars(spin.while_value.value.scalar_type(), |value| {
                    observe(Some(value))
                })
                .is_none()
            {
                observe(None);
            }
            if can_block {
                let cause = if all_block && certain {
                    ProgressFailure::Deadlock
                } else {
                    ProgressFailure::Unproved
                };
                return Some(Advance::Blocked(PrivateWaitFailure {
                    machine: frame.machine.id,
                    operation: op.id,
                    cause,
                }));
            }
            if spin.update != CellUpdate::Observe {
                let next = memory.get(&location).after_update(spin.update);
                memory.set(&location, next);
            }
            continue;
        }
        let (successor, cleanup) = match &frame.block.terminator {
            Terminator::Jump {
                target,
                arguments,
                structural_arguments,
                cleanup_actions,
                ..
            } => (
                Some((
                    *target,
                    ScalarArguments::Values(arguments),
                    structural_arguments,
                )),
                cleanup_actions,
            ),
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let answer = match frame.bindings.values.get(condition) {
                    Some(ScalarFact::Literal(Scalar::Boolean(value))) => *value,
                    _ => match frame.selected_arm {
                        Some(value) => {
                            frame
                                .bindings
                                .values
                                .insert(*condition, ScalarFact::Literal(Scalar::Boolean(value)));
                            value
                        }
                        None => return Some(Advance::Fork),
                    },
                };
                let edge = if answer { when_true } else { when_false };
                (
                    Some((
                        edge.target,
                        ScalarArguments::Values(&edge.arguments),
                        &edge.structural_arguments,
                    )),
                    &edge.cleanup_actions,
                )
            }
            Terminator::StructuralCase {
                source,
                path,
                cases,
            } => {
                let (edge, fields) =
                    aggregate::dispatch(module, frame, memory, *source, path, cases)?;
                (
                    Some((
                        edge.target,
                        ScalarArguments::Payload(fields),
                        &edge.structural_arguments,
                    )),
                    &edge.cleanup_actions,
                )
            }
            Terminator::ReturnUnit {
                cleanup_actions, ..
            }
            | Terminator::Return {
                cleanup_actions, ..
            }
            | Terminator::ReturnStructural {
                cleanup_actions, ..
            } => (None, cleanup_actions),
            _ => return None,
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
        if let Some((target, scalars, places)) = successor {
            frame.jump(target, scalars, places, memory)?;
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
                    let returned = memory.allocate(memory.get(location).clone());
                    caller.bindings.places.insert(dest, returned);
                }
                _ => return None,
            }
            return Some(Advance::Continue);
        }
    }
    None
}

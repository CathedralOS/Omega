//! Instantiate derived wait predicates on fresh, unescaped storage. Borrow
//! arguments identify the same allocation across checked calls and jumps.

mod activation;
mod storage;

use super::facts::Facts;
use super::selected::Summaries;
use super::spin::SpinWait;
use crate::entry_values::TerminalScalarValue as Scalar;
use activation::{Bindings, Destination, Frame};
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
use storage::{Storage, Stored};
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
    if !module.types.structural_types.iter().any(|ty| {
        matches!(ty.shape, terminal_psi::StructuralTypeShape::AtomicCell(_))
            || ty
                .shape
                .fields()
                .any(|field| field.field_type.atomic_cell_type().is_some())
    }) {
        return None;
    }
    let mut stack = vec![Frame::enter(
        module.machine(entry)?,
        Bindings::default(),
        Destination::Unit,
    )?];
    let mut memory = Storage::default();
    let mut seen = Facts::default();
    let mut summaries = Summaries::default();
    while let Some(frame) = stack.last_mut() {
        if frame.position == 0
            && frame.cleanup_cursor == 0
            && seen
                .insert((frame.machine.id, frame.block.id), ())
                .is_some()
        {
            return None;
        }
        if let Some(op) = frame.block.operations.get(frame.position) {
            frame.position += 1;
            if op.suspension_crossing.is_some() {
                return None;
            }
            let Some(call) = op.declared_invocation() else {
                frame.step(module, op, &mut memory)?;
                continue;
            };
            let spin = match call.callee {
                TargetMachineCallable::Machine(id) => {
                    if let Some(summary) = summaries.get(module, id, &boundary) {
                        summary
                    } else {
                        let callee = module.machine(id)?;
                        let args = frame.bindings.arguments(
                            &mut memory,
                            &callee.parameters,
                            &callee.structural_parameters,
                            call.arguments,
                            call.structural_arguments,
                        )?;
                        stack.push(Frame::enter(callee, args, Destination::of(op))?);
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
            let value = memory
                .get(&location)
                .scalar(spin.while_value.value.scalar_type())?;
            if spin.while_value.matches(value)
                && spin
                    .writes
                    .is_none_or(|next| spin.while_value.matches(next))
            {
                return Some((frame.machine.id, op.id));
            }
            if let Some(value) = spin.writes {
                memory.set(&location, Stored::Scalar(value));
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
                Some((*target, arguments, structural_arguments)),
                cleanup_actions,
            ),
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let Scalar::Boolean(test) = frame.bindings.values.get(condition)? else {
                    return None;
                };
                let edge = if *test { when_true } else { when_false };
                (
                    Some((edge.target, &edge.arguments, &edge.structural_arguments)),
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
                    &mut memory,
                    &callee.parameters,
                    &callee.structural_parameters,
                    &[],
                    std::slice::from_ref(&invocation.subject),
                )?;
                stack.push(Frame::enter(callee, args, Destination::Unit)?);
            }
            continue;
        }
        if let Some((target, scalars, places)) = successor {
            frame.jump(target, scalars, places, &mut memory)?;
            continue;
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
        }
    }
    None
}

//! Select the successor and capture its arguments once. Edge cleanup runs
//! after this read and before the destination block starts.

use super::activation::{Frame, ScalarArguments};
use super::aggregate;
use super::storage::Storage;
use crate::entry_values::TerminalScalarValue as Scalar;
use crate::synchronization::progress::ScalarFact;
use std::borrow::Cow;
use terminal_psi::{TerminalAffineCleanupAction, TerminalModule, Terminator};

pub(super) enum Selection<'m> {
    Fork(Cow<'static, [usize]>),
    Ready(&'m [TerminalAffineCleanupAction]),
}

pub(super) fn prepare<'m>(
    module: &'m TerminalModule,
    frame: &mut Frame<'m>,
    memory: &mut Storage,
) -> Option<Selection<'m>> {
    if let Some(arrival) = &frame.arrival {
        return Some(Selection::Ready(arrival.cleanup));
    }
    let (edge, target, scalars, places, cleanup) = match &frame.block.terminator {
        Terminator::Jump {
            edge,
            target,
            arguments,
            structural_arguments,
            cleanup_actions,
            ..
        } => (
            *edge,
            *target,
            ScalarArguments::Values(arguments),
            structural_arguments,
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
                    Some(arm) => {
                        let value = arm == 0;
                        if let Some(test) = frame.bindings.predicate(*condition) {
                            memory.assume(test.selected(value));
                        }
                        frame
                            .bindings
                            .values
                            .insert(*condition, ScalarFact::Literal(Scalar::Boolean(value)));
                        value
                    }
                    None => {
                        return Some(Selection::Fork(Cow::Borrowed(&[0, 1])));
                    }
                },
            };
            let edge = if answer { when_true } else { when_false };
            (
                edge.edge,
                edge.target,
                ScalarArguments::Values(&edge.arguments),
                &edge.structural_arguments,
                &edge.cleanup_actions,
            )
        }
        Terminator::StructuralCase {
            source,
            path,
            cases,
        } => {
            let (edge, fields) =
                match aggregate::dispatch(module, frame, memory, *source, path, cases)? {
                    aggregate::Dispatch::Fork(arms) => {
                        return Some(Selection::Fork(arms.into()));
                    }
                    aggregate::Dispatch::Selected(edge, fields) => (edge, fields),
                };
            (
                edge.edge,
                edge.target,
                ScalarArguments::Payload(fields),
                &edge.structural_arguments,
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
        } => return Some(Selection::Ready(cleanup_actions)),
        _ => return None,
    };
    frame.prepare_arrival(edge, target, scalars, places, cleanup, memory)?;
    Some(Selection::Ready(cleanup))
}

#[cfg(test)]
mod tests;

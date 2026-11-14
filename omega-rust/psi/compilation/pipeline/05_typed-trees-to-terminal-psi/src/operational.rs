//! Preserve source acknowledgements before calls become SSA operations or
//! loop edges. Source position is a formation rule: lifting an argument into
//! a temporary does not turn a nested `suspend` into a legal suspension point.

use semantic_vocabulary::MachineId;
use terminal_check_tables::Checked;
use terminal_psi::{CallAcknowledgement, CallOrigin};

use crate::body::{Body, Expression, ExpressionKind, Selector, Statement, Target};

pub(crate) fn retain(
    expression: &mut Expression,
    acknowledgement: language_semantics::CallOperationalAcknowledgement,
    suspension_position: bool,
) {
    expression.call_context = Some(match acknowledgement.origin {
        language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized => {
            CallAcknowledgement {
                suspension_position,
                ..CallAcknowledgement::generated()
            }
        }
        language_semantics::CallOperationalAcknowledgementOrigin::Source => CallAcknowledgement {
            origin: CallOrigin::Authored,
            acknowledges_suspend: acknowledgement.acknowledges_suspend,
            acknowledges_block: acknowledgement.acknowledges_block,
            suspension_position,
        },
    });
}

pub(crate) fn boundary(expression: &mut Expression) {
    if let Some(context) = &mut expression.call_context {
        context.suspension_position = true;
    }
}

pub(crate) fn positions(machine: MachineId, body: &Body) -> Checked<()> {
    fn target(machine: MachineId, target: &Target) -> Checked<()> {
        if let Target::Value(value) = target {
            expression(machine, value)?;
        }
        Ok(())
    }
    for statement in &body.statements {
        match statement {
            Statement::Bind { value, .. }
            | Statement::Evaluate(value)
            | Statement::Witness { term: value, .. } => expression(machine, value)?,
            Statement::Store {
                destination, value, ..
            } => {
                expression(machine, destination)?;
                expression(machine, value)?;
            }
            Statement::Dispatch {
                subject,
                arms,
                default,
                ..
            } => {
                expression(machine, subject)?;
                for (_, arm, _) in arms {
                    target(machine, arm)?;
                }
                if let Some((arm, _)) = default {
                    target(machine, arm)?;
                }
            }
            Statement::Arm {
                guard, target: arm, ..
            } => {
                if let Some(guard) = guard {
                    expression(machine, guard)?;
                }
                target(machine, arm)?;
            }
            Statement::Declare { .. } | Statement::CaptureReference { .. } => {}
        }
    }
    Ok(())
}

fn expression(machine: MachineId, value: &Expression) -> Checked<()> {
    if value.call_context.is_some_and(|context| {
        context.origin == CallOrigin::Authored
            && context.acknowledges_suspend
            && !context.suspension_position
    }) {
        return Err(crate::reject(machine, value.source,
            "a suspend-marked call must be a complete statement, simple let initializer, transition subject or terminal expression".to_owned()));
    }
    match &value.kind {
        ExpressionKind::Integer(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::ByteLiteral(_)
        | ExpressionKind::Slot(_)
        | ExpressionKind::SavedSubject(_) => {}
        ExpressionKind::Not(operand)
        | ExpressionKind::BitwiseNot(operand)
        | ExpressionKind::Referent(operand)
        | ExpressionKind::Cast(operand)
        | ExpressionKind::Requalify(operand)
        | ExpressionKind::Establish { operand, .. }
        | ExpressionKind::Field { base: operand, .. }
        | ExpressionKind::Payload { base: operand, .. }
        | ExpressionKind::CollectionMeasure { base: operand, .. }
        | ExpressionKind::IsCase {
            subject: operand, ..
        }
        | ExpressionKind::Borrow { place: operand, .. } => expression(machine, operand)?,
        ExpressionKind::Array { elements }
        | ExpressionKind::Call {
            arguments: elements,
            ..
        } => {
            for element in elements {
                expression(machine, element)?;
            }
        }
        ExpressionKind::Index { base, index } | ExpressionKind::ByteElement { base, index } => {
            expression(machine, base)?;
            expression(machine, index)?;
        }
        ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
            for (_, field) in fields {
                expression(machine, field)?;
            }
        }
        ExpressionKind::Match {
            subject,
            arms,
            default,
            ..
        } => {
            expression(machine, subject)?;
            for (selector, arm) in arms {
                if let Selector::Value(pattern) = selector {
                    expression(machine, pattern)?;
                }
                expression(machine, arm)?;
            }
            if let Some(arm) = default {
                expression(machine, arm)?;
            }
        }
        ExpressionKind::Strict { left, right, .. }
        | ExpressionKind::ShortCircuit { left, right, .. } => {
            expression(machine, left)?;
            expression(machine, right)?;
        }
    }
    Ok(())
}

//! Preserve source acknowledgements before calls become SSA operations or
//! loop edges. Source position is a formation rule: lifting an argument into
//! a temporary does not turn a nested `suspend` into a legal suspension point.

use semantic_vocabulary::MachineId;
use terminal_check_tables::Checked;
use terminal_psi::{CallAcknowledgement, CallOrigin};

use crate::lowering::body::{Body, Expression, ExpressionKind, Statement, Target};

pub(crate) fn retain(
    expression: &mut Expression,
    acknowledgement: language_core::CallOperationalAcknowledgement,
    suspension_position: bool,
) {
    expression.call_context = Some(match acknowledgement.origin {
        language_core::CallOperationalAcknowledgementOrigin::CompilerSynthesized => {
            CallAcknowledgement {
                suspension_position,
                ..CallAcknowledgement::generated()
            }
        }
        language_core::CallOperationalAcknowledgementOrigin::Source => CallAcknowledgement {
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

/// Resolve the parsed target token while the original typed call is still available.
pub(crate) fn retain_callee_spans(
    typed: &crate::source_state::TypedTrees,
    lowered: &mut super::lower::Lowered,
) {
    use crate::source_state::{SourceNode, statement::StatementNode};
    use semantic_vocabulary::InvocationSite;
    use terminal_psi::CallSite;

    let callee_span = |node| {
        let occurrence = match node {
            SourceNode::Expression(id) if typed.expression_table.expression_is_valid(id) => {
                typed.expression_table.call_node(id)?.authored_call_selection
            }
            SourceNode::Statement(id) if typed.statement_table.statement_is_valid(id) => {
                let StatementNode::Call(call) = typed.statement_table.statement(id) else {
                    return None;
                };
                call.authored_call_selection
            }
            _ => None,
        }?;
        let selection = typed.authored_declaration_selections().get(occurrence)?;
        (selection.kind() == crate::declaration_selection::AuthoredDeclarationSelectionKind::Call)
            .then(|| selection.source_span())
    };
    for call in &lowered.source_calls {
        let site = match call.site {
            InvocationSite::Operation(id) => CallSite::Operation(id),
            InvocationSite::InvokeNoNormal(id) | InvocationSite::ExternalCompletion(id) => {
                CallSite::Edge(id)
            }
            InvocationSite::Cleanup { .. } => continue,
        };
        if let Some(span) = callee_span(call.source) {
            lowered.sources.call_targets.push((site, span));
        }
    }
    for &(_, edge, _, node) in &lowered.source_folded_calls {
        if let Some(span) = callee_span(node) {
            lowered
                .sources
                .call_targets
                .push((CallSite::Edge(edge), span));
        }
    }
}

pub(crate) fn positions(machine: MachineId, body: &Body) -> Checked<()> {
    statements(machine, &body.statements)
}

fn statements(machine: MachineId, statements: &[Statement]) -> Checked<()> {
    fn target(machine: MachineId, target: &Target) -> Checked<()> {
        if let Target::Value(value) = target {
            expression(machine, value)?;
        }
        Ok(())
    }
    for statement in statements {
        match statement {
            Statement::Observe { value, .. }
            | Statement::Bind { value, .. }
            | Statement::Decompose { subject: value }
            | Statement::Evaluate {
                expression: value, ..
            }
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
                for (selector, arm, _) in arms {
                    if let Some(condition) = selector.condition.as_deref() {
                        expression(machine, condition)?;
                    }
                    target(machine, arm)?;
                }
                if let Some((arm, _)) = default {
                    target(machine, arm)?;
                }
            }
            Statement::Arm {
                guard, target: arm, ..
            } => {
                if let Some(guard) = guard.condition() {
                    expression(machine, guard)?;
                }
                target(machine, arm)?;
            }
            Statement::Declare { .. }
            | Statement::CaptureReference { .. }
            | Statement::ParkedBuildLocals(..) => {}
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
        return Err(crate::lowering::reject(machine, value.source,
            "a suspend-marked call must be a complete statement, simple let initializer, transition subject or terminal expression".to_owned()));
    }
    match &value.kind {
        ExpressionKind::TheoremApplication { .. } => {}
        ExpressionKind::Atomic(operation) => {
            for operand in operation.inputs() {
                expression(machine, operand)?;
            }
        }
        ExpressionKind::Integer(_)
        | ExpressionKind::Float(_)
        | ExpressionKind::ErasedApplication(_)
        | ExpressionKind::ScalarApplication { .. }
        | ExpressionKind::Zero
        | ExpressionKind::Boolean(_)
        | ExpressionKind::ByteLiteral(_)
        | ExpressionKind::Slot(_)
        | ExpressionKind::SavedSubject(_)
        | ExpressionKind::MachineValue { .. } => {}
        ExpressionKind::OwnedTemporary { value: operand, .. }
        | ExpressionKind::Not(operand)
        | ExpressionKind::BitwiseNot(operand)
        | ExpressionKind::Referent(operand)
        | ExpressionKind::Cast(operand)
        | ExpressionKind::Requalify(operand)
        | ExpressionKind::Establish { operand, .. }
        | ExpressionKind::Field { base: operand, .. }
        | ExpressionKind::Payload { base: operand, .. }
        | ExpressionKind::CollectionView { base: operand, .. }
        | ExpressionKind::StorageLocation { place: operand, .. }
        | ExpressionKind::CollectionMeasure { base: operand, .. }
        | ExpressionKind::ByteSequencePredicate { base: operand, .. }
        | ExpressionKind::IsCase {
            subject: operand, ..
        }
        | ExpressionKind::Borrow { place: operand, .. } => expression(machine, operand)?,
        ExpressionKind::Call {
            callee, arguments, ..
        } => {
            if let crate::lowering::body::CallTarget::Selected { callable, .. } = callee {
                expression(machine, callable)?;
            }
            for argument in arguments {
                expression(machine, argument)?;
            }
        }
        ExpressionKind::Array { elements } => {
            for element in elements {
                expression(machine, element)?;
            }
        }
        ExpressionKind::CollectionSubslice { base, start, end } => {
            for operand in std::iter::once(base.as_ref())
                .chain(start.as_deref())
                .chain(end.as_deref())
            {
                expression(machine, operand)?;
            }
        }
        ExpressionKind::Index { base, index } | ExpressionKind::ByteElement { base, index, .. } => {
            expression(machine, base)?;
            expression(machine, index)?;
        }
        ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
            for (_, field) in fields {
                expression(machine, field)?;
            }
        }
        ExpressionKind::Block {
            statements: inner,
            value,
            ..
        } => {
            statements(machine, inner)?;
            if let Some(value) = value {
                expression(machine, value)?;
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
                if let Some(pattern) = selector.test() {
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

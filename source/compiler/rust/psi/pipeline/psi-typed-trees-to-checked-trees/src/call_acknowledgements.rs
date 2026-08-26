use psi_diagnostics::Diagnostic;
use psi_effects::{CallOperational, OperationalPlan};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode};

/// Validate the source acknowledgement set against the call envelope already
/// normalized by `psi-effects`, and reject suspension in a nested expression
/// before continuation planning sees it.
pub(crate) fn validate_call_acknowledgements(
    program: &TypedTrees,
    operational: &OperationalPlan,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let Some(operational_state) = operational
                .machines()
                .iter()
                .flat_map(|machine| operational.states.span_or_empty(machine.states))
                .find(|summary| summary.symbol == state.symbol)
            else {
                continue;
            };
            let operational_calls = operational.calls.span_or_empty(operational_state.calls);
            let statements = program.statement_table.statements(state.statement_nodes);

            for (statement_index, statement) in statements.iter().enumerate() {
                let mut call_ordinal = 0usize;
                let terminal = statement_index + 1 == statements.len();
                validate_statement(
                    program,
                    statement,
                    statement_index,
                    terminal,
                    operational_calls,
                    &mut call_ordinal,
                    &mut diagnostics,
                );
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_statement(
    program: &TypedTrees,
    statement: &StatementNode,
    statement_index: usize,
    terminal: bool,
    operational_calls: &[CallOperational],
    call_ordinal: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            validate_expression(
                program,
                assignment.target,
                statement_index,
                false,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
            validate_expression(
                program,
                assignment.value,
                statement_index,
                false,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
        }
        StatementNode::Call(call) => {
            validate_call(
                call.target.as_str(),
                statement_index,
                true,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
            for argument in program.statement_table.expression_handles(call.arguments) {
                validate_expression(
                    program,
                    *argument,
                    statement_index,
                    false,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                );
            }
        }
        StatementNode::Expression(expression) => validate_expression(
            program,
            *expression,
            statement_index,
            terminal,
            operational_calls,
            call_ordinal,
            diagnostics,
        ),
        StatementNode::LocalData(local) if local.initial_value.is_valid() => validate_expression(
            program,
            local.initial_value,
            statement_index,
            true,
            operational_calls,
            call_ordinal,
            diagnostics,
        ),
        StatementNode::LocalData(_) => {}
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                validate_expression(
                    program,
                    guard,
                    statement_index,
                    true,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
    statement_index: usize,
    direct_position: bool,
    operational_calls: &[CallOperational],
    call_ordinal: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => validate_expression(
            program,
            atomic.value,
            statement_index,
            false,
            operational_calls,
            call_ordinal,
            diagnostics,
        ),
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                validate_expression(
                    program,
                    *value,
                    statement_index,
                    false,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            validate_expression(
                program,
                binary.left,
                statement_index,
                false,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
            validate_expression(
                program,
                binary.right,
                statement_index,
                false,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
        }
        ExpressionNode::Cast(cast) => validate_expression(
            program,
            cast.value,
            statement_index,
            false,
            operational_calls,
            call_ordinal,
            diagnostics,
        ),
        ExpressionNode::Call(call) => {
            validate_call(
                call.target.as_str(),
                statement_index,
                direct_position,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
            validate_expression(
                program,
                call.receiver,
                statement_index,
                false,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                validate_expression(
                    program,
                    *argument,
                    statement_index,
                    false,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Indexed(indexed) => {
            validate_expression(
                program,
                indexed.collection,
                statement_index,
                false,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
            validate_expression(
                program,
                indexed.index,
                statement_index,
                false,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
        }
        ExpressionNode::Member(member) => validate_expression(
            program,
            member.receiver,
            statement_index,
            false,
            operational_calls,
            call_ordinal,
            diagnostics,
        ),
        ExpressionNode::Borrow(inner) => validate_expression(
            program,
            inner.target,
            statement_index,
            false,
            operational_calls,
            call_ordinal,
            diagnostics,
        ),
        ExpressionNode::Range(range) => {
            validate_expression(
                program,
                range.start,
                statement_index,
                false,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
            validate_expression(
                program,
                range.end,
                statement_index,
                false,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                validate_expression(
                    program,
                    field.value,
                    statement_index,
                    false,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Unary(unary) => validate_expression(
            program,
            unary.operand,
            statement_index,
            false,
            operational_calls,
            call_ordinal,
            diagnostics,
        ),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn validate_call(
    target: &str,
    statement_index: usize,
    direct_position: bool,
    operational_calls: &[CallOperational],
    call_ordinal: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ordinal = *call_ordinal;
    *call_ordinal = call_ordinal.checked_add(1).expect("call ordinal overflow");
    let Some(call) = operational_calls
        .iter()
        .find(|call| call.statement_index == statement_index && call.call_ordinal == ordinal)
    else {
        return;
    };

    let acknowledgement = call.acknowledgement;
    let may_suspend = call.transitive_may_suspend;
    let may_block = call.transitive_may_block;
    if acknowledgement.acknowledges_suspend != may_suspend
        || acknowledgement.acknowledges_block != may_block
    {
        diagnostics.push(Diagnostic::error(format!(
            "call to `{target}` has operational envelope {} but acknowledges {}; call acknowledgements must match exactly",
            envelope_label(may_suspend, may_block),
            envelope_label(
                acknowledgement.acknowledges_suspend,
                acknowledgement.acknowledges_block,
            ),
        )));
    }

    if may_suspend && !direct_position {
        diagnostics.push(Diagnostic::error(format!(
            "suspending call to `{target}` is nested inside a partially evaluated expression; bind it as a complete statement, simple `let` right-hand side, transition subject, or terminal expression before continuation planning",
        )));
    }
}

fn envelope_label(may_suspend: bool, may_block: bool) -> &'static str {
    match (may_suspend, may_block) {
        (false, false) => "neither suspension nor blocking",
        (true, false) => "`suspend`",
        (false, true) => "`block`",
        (true, true) => "`suspend block`",
    }
}

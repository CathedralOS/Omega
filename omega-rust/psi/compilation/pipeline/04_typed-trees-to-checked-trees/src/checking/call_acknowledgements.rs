use diagnostics::Diagnostic;
use flow_effects::{CallOperational, OperationalPlan};
use symbols::JoinedPath;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

/// Validate the source acknowledgement set against the call envelope already
/// normalized by `flow-effects`, and reject suspension in a nested expression
/// before continuation planning sees it.
pub(crate) fn validate_call_acknowledgements(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    statement: &StatementNode,
    statement_index: usize,
    terminal: bool,
    operational_calls: &[CallOperational],
    call_ordinal: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::RootBinding(binding) => {
            for expression in [binding.receiver, binding.implementation_operand] {
                if expression.is_valid() {
                    // Binding still has work to do after its operand returns;
                    // it is not a terminal suspension/call position.
                    validate_child(
                        program,
                        expression,
                        statement_index,
                        operational_calls,
                        call_ordinal,
                        diagnostics,
                    );
                }
            }
        }
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            validate_child(
                program,
                assignment.target,
                statement_index,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
            validate_child(
                program,
                assignment.value,
                statement_index,
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
                validate_child(
                    program,
                    *argument,
                    statement_index,
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
            for target in transition.targets() {
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        let target_name = program
                            .statement_table
                            .name_path_members(path.members)
                            .joined_path();
                        // Named transfers reserve a parent coordinate even
                        // when internal control flow has no operational call.
                        validate_call(
                            &target_name,
                            statement_index,
                            true,
                            operational_calls,
                            call_ordinal,
                            diagnostics,
                        );
                        for argument in program.statement_table.expression_handles(*arguments) {
                            validate_child(
                                program,
                                *argument,
                                statement_index,
                                operational_calls,
                                call_ordinal,
                                diagnostics,
                            );
                        }
                    }
                    TransitionTargetNode::Value(expression) => validate_expression(
                        program,
                        *expression,
                        statement_index,
                        true,
                        operational_calls,
                        call_ordinal,
                        diagnostics,
                    ),
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]

/// Every recursive child walk spells non-direct position identically.
fn validate_child(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    statement_index: usize,
    operational_calls: &[CallOperational],
    call_ordinal: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_expression(
        program,
        expression,
        statement_index,
        false,
        operational_calls,
        call_ordinal,
        diagnostics,
    );
}

fn validate_expression(
    program: &dyn ::validation::CheckingInput,
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
        ExpressionNode::Match(dispatch) => {
            validate_child(
                program,
                dispatch.subject,
                statement_index,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
            for arm in program.expression_table.match_arms(dispatch.arms) {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    validate_child(
                        program,
                        pattern,
                        statement_index,
                        operational_calls,
                        call_ordinal,
                        diagnostics,
                    );
                }
                validate_child(
                    program,
                    arm.value,
                    statement_index,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Atomic(atomic) => validate_child(
            program,
            atomic.value,
            statement_index,
            operational_calls,
            call_ordinal,
            diagnostics,
        ),
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                validate_child(
                    program,
                    *value,
                    statement_index,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            validate_child(
                program,
                binary.left,
                statement_index,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
            validate_child(
                program,
                binary.right,
                statement_index,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
        }
        ExpressionNode::Cast(cast) => validate_child(
            program,
            cast.value,
            statement_index,
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
            validate_child(
                program,
                call.receiver,
                statement_index,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                validate_child(
                    program,
                    *argument,
                    statement_index,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Indexed(indexed) => {
            validate_child(
                program,
                indexed.collection,
                statement_index,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
            validate_child(
                program,
                indexed.index,
                statement_index,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
        }
        ExpressionNode::Member(member) => validate_child(
            program,
            member.receiver,
            statement_index,
            operational_calls,
            call_ordinal,
            diagnostics,
        ),
        ExpressionNode::Borrow(inner) => validate_child(
            program,
            inner.target,
            statement_index,
            operational_calls,
            call_ordinal,
            diagnostics,
        ),
        ExpressionNode::Range(range) => {
            validate_child(
                program,
                range.start,
                statement_index,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
            validate_child(
                program,
                range.end,
                statement_index,
                operational_calls,
                call_ordinal,
                diagnostics,
            );
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                validate_child(
                    program,
                    field.value,
                    statement_index,
                    operational_calls,
                    call_ordinal,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Unary(unary) => validate_child(
            program,
            unary.operand,
            statement_index,
            operational_calls,
            call_ordinal,
            diagnostics,
        ),
        typed_trees::literal_expression_pattern!() => {}
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

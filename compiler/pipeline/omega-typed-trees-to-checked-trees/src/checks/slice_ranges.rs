use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::expression::ExpressionNode;
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

pub(crate) fn reject_slice_ranges(
    program: &omega_typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Assignment(assignment) => {
                        collect_range_diagnostics(program, assignment.target, &mut diagnostics);
                        collect_range_diagnostics(program, assignment.value, &mut diagnostics);
                    }
                    StatementNode::Call(call) => {
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            collect_range_diagnostics(program, *argument, &mut diagnostics);
                        }
                    }
                    StatementNode::Expression(expression) => {
                        collect_range_diagnostics(program, *expression, &mut diagnostics);
                    }
                    StatementNode::LocalData(local) => {
                        if local.initial_value.is_valid() {
                            collect_range_diagnostics(
                                program,
                                local.initial_value,
                                &mut diagnostics,
                            );
                        }
                    }
                    StatementNode::Transition(transition) => {
                        collect_transition_target_ranges(
                            program,
                            program.statement_table.transition_target(transition.target),
                            &mut diagnostics,
                        );
                        collect_transition_target_ranges(
                            program,
                            program.statement_table.transition_target(transition.continuation),
                            &mut diagnostics,
                        );
                        if let TransitionGuardNode::When(guard) = transition.guard {
                            collect_range_diagnostics(program, guard, &mut diagnostics);
                        }
                    }
                }
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn collect_transition_target_ranges(
    program: &omega_typed_trees::TypedTrees,
    target: &TransitionTargetNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match target {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                collect_range_diagnostics(program, *argument, diagnostics);
            }
        }
        TransitionTargetNode::Value(value) => {
            collect_range_diagnostics(program, *value, diagnostics);
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn collect_range_diagnostics(
    program: &omega_typed_trees::TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_range_diagnostics(program, *value, diagnostics);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_range_diagnostics(program, binary.left, diagnostics);
            collect_range_diagnostics(program, binary.right, diagnostics);
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
        ExpressionNode::Cast(cast) => {
            collect_range_diagnostics(program, cast.value, diagnostics);
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_range_diagnostics(program, call.receiver, diagnostics);
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_range_diagnostics(program, *argument, diagnostics);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_range_diagnostics(program, indexed.collection, diagnostics);
            collect_range_diagnostics(program, indexed.index, diagnostics);
        }
        ExpressionNode::Member(member) => {
            collect_range_diagnostics(program, member.receiver, diagnostics);
        }
        ExpressionNode::Mutable(inner) => {
            collect_range_diagnostics(program, *inner, diagnostics);
        }
        ExpressionNode::Range(_) => diagnostics.push(Diagnostic::error(
            "slice ranges are not implemented yet",
        )),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program.expression_table.struct_fields(struct_literal.fields) {
                collect_range_diagnostics(program, field.value, diagnostics);
            }
        }
    }
}

//! Public runtime Value projections use the ordinary call operand lane.
//! Static/evidence arguments close a reusable declaration; each invocation
//! still evaluates and passes its own Values, independently of that key.

use crate::tables::{expressions_of, expressions_of_mut};
use diagnostics::Diagnostic;
use typed_trees::{TypedTrees, expression::ExpressionNode, statement::StatementNode};

pub(super) fn realize(
    program: &mut TypedTrees,
    invocation: &super::calls::PreparedInvocation,
) -> Result<(), Vec<Diagnostic>> {
    let (target, count) = match invocation.site {
        super::super::CallSite::Expression(expression) => {
            let Some(call) = expressions_of(program).call_node(expression) else {
                return Ok(());
            };
            (call.target_symbol, call.arguments.len() as usize)
        }
        super::super::CallSite::Statement(statement) => {
            let Some(call) = program.statement_table.statement(statement).as_call() else {
                return Ok(());
            };
            (call.target_symbol, call.arguments.len() as usize)
        }
    };
    let Some(application) = program.closed_requirement_application(target) else {
        return Ok(());
    };
    let expected = program
        .state_signature_parameters(application.signature)
        .len();
    let runtime = application
        .local_arguments
        .iter()
        .zip(&invocation.tuple.arguments)
        .filter_map(|(coordinate, actual)| match coordinate {
            typed_trees::typed_trees::RequirementLocalArgument::RuntimeValue { .. } => {
                Some(super::super::RuntimeValueBinding::Authored(actual.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if runtime.is_empty() || count == expected {
        return Ok(());
    }
    if count + runtime.len() != expected {
        return Err(vec![Diagnostic::error(
            "runtime requirement projection lost its ordinary operand roster",
        )]);
    }
    let operands =
        super::super::body_rewriting::runtime_value_operands(program, invocation.state, &runtime);
    if operands.len() != runtime.len() {
        return Err(vec![Diagnostic::error(
            "runtime requirement projection lost an executable Value operand",
        )]);
    }
    if let Some(application) = invocation.site.authored_machine_application(program) {
        super::super::body_rewriting::stamp_field_subjects(program, application, &operands);
    }
    match invocation.site {
        super::super::CallSite::Expression(expression) => {
            let snapshot = expressions_of(program).call_node(expression).unwrap();
            let mut arguments = expressions_of(program)
                .expression_handles(snapshot.arguments)
                .to_vec();
            arguments.extend(operands);
            let arguments = expressions_of_mut(program).insert_expression_handles(arguments);
            let ExpressionNode::Call(call) = expressions_of_mut(program).expression_mut(expression)
            else {
                unreachable!()
            };
            call.arguments = arguments;
        }
        super::super::CallSite::Statement(statement) => {
            let arguments = program
                .statement_table
                .statement(statement)
                .as_call()
                .unwrap()
                .arguments;
            let arguments = super::super::body_rewriting::insert_arguments_with_subjects(
                program, arguments, &operands,
            );
            let StatementNode::Call(call) = program.statement_table.statement_mut(statement) else {
                unreachable!()
            };
            call.arguments = arguments;
        }
    }
    Ok(())
}

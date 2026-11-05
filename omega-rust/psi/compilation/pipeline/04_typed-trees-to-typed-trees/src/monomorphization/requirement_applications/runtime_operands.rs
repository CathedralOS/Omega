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
    let (target, arguments) = match invocation.site {
        super::super::CallSite::Expression(expression) => {
            let Some(call) = expressions_of(program).call_node(expression) else {
                return Ok(());
            };
            (call.target_symbol, call.arguments)
        }
        super::super::CallSite::Statement(statement) => {
            let Some(call) = program.statement_table.statement(statement).as_call() else {
                return Ok(());
            };
            (call.target_symbol, call.arguments)
        }
    };
    let count = arguments.len();
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
    if runtime.is_empty() {
        return Ok(());
    }
    // Preparation may revisit this occurrence after rebuilding its static
    // bindings. Its already evaluated Value operands still own those bindings;
    // returning on arity alone would lose fields, which have no direct symbol.
    // The closed signature's exact binder suffix authenticates the positions.
    let (_, declared) = program
        .trait_machine_signature(application.template)
        .ok_or_else(|| super::super::failed("runtime requirement lost its authored signature"))?;
    let ordinary = program
        .applied_signature_authored_parameters(declared, application.signature)
        .ok_or_else(|| {
            super::super::failed("runtime requirement lost its Value formal correspondence")
        })?
        .len();
    if ordinary + runtime.len() != expected {
        return Err(super::super::failed(
            "runtime requirement projection lost its complete Value formal roster",
        ));
    }
    if count == expected {
        let operands = match invocation.site {
            super::super::CallSite::Expression(_) => {
                expressions_of(program).expression_handles(arguments)[ordinary..].to_vec()
            }
            super::super::CallSite::Statement(_) => {
                program.statement_table.expression_handles(arguments)[ordinary..].to_vec()
            }
        };
        if let Some(application) = invocation.site.authored_machine_application(program) {
            super::super::body_rewriting::record_runtime_call_operands(
                program,
                application,
                &operands,
            );
        }
        return Ok(());
    }
    if count + runtime.len() != expected {
        return Err(vec![Diagnostic::error(
            "runtime requirement projection lost its ordinary operand roster",
        )]);
    }
    let operands = super::super::body_rewriting::runtime_call_operands(
        program,
        invocation.site,
        invocation.state,
        &runtime,
    );
    if operands.len() != runtime.len() {
        return Err(vec![Diagnostic::error(
            "runtime requirement projection lost an executable Value operand",
        )]);
    }
    if let Some(application) = invocation.site.authored_machine_application(program) {
        super::super::body_rewriting::record_runtime_call_operands(program, application, &operands);
    }
    match invocation.site {
        super::super::CallSite::Expression(expression) => {
            let mut arguments = expressions_of(program)
                .expression_handles(arguments)
                .to_vec();
            arguments.extend(operands);
            let arguments = expressions_of_mut(program).insert_expression_handles(arguments);
            let ExpressionNode::Call(call) = expressions_of_mut(program).expression_mut(expression)
            else {
                return Err(super::super::failed(
                    "runtime requirement projection lost its expression call",
                ));
            };
            call.arguments = arguments;
        }
        super::super::CallSite::Statement(statement) => {
            let arguments = super::super::body_rewriting::insert_arguments_with_subjects(
                program, arguments, &operands,
            );
            let StatementNode::Call(call) = program.statement_table.statement_mut(statement) else {
                return Err(super::super::failed(
                    "runtime requirement projection lost its statement call",
                ));
            };
            call.arguments = arguments;
        }
    }
    Ok(())
}

//! Complete projected receiver calls before validation and effect inference.

use diagnostics::Diagnostic;
use typed_trees::{TypedTrees, statement::StatementNode};

use validation::{declared_receiver_target, receiver_projections as projections};

pub(crate) fn resolve_projected_receiver_calls(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut updates = Vec::new();
    let mut expression_updates = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let statements = program.statement_table.statements(state.statement_nodes);
            for (index, statement) in statements.iter().enumerate() {
                let mut expressions = Vec::new();
                validation::expression_traversal::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                for expression in expressions {
                    let typed_trees::expression::ExpressionNode::Call(call) =
                        program.expression_table.expression(expression)
                    else {
                        continue;
                    };
                    let Some(receiver) = projections::expression(
                        program,
                        state,
                        index,
                        call.receiver,
                        &mut Vec::new(),
                    )?
                    else {
                        continue;
                    };
                    if receiver.depth == 0 {
                        continue;
                    }
                    let target = declared_receiver_target(
                        program,
                        receiver.type_reference,
                        call.target_symbol,
                        &call.target,
                        program.expression_table.source_span(expression),
                    )?;
                    if target.is_valid() {
                        expression_updates.push((expression, target));
                    }
                }
                let Some(call) = statement.as_call() else {
                    continue;
                };
                let members = program.statement_table.name_path_members(call.receiver);
                if members.len() < 2 || !call.receiver_root_symbol.is_valid() {
                    continue;
                }
                let Some(root) = projections::root(
                    program,
                    state,
                    index,
                    call.receiver_root_symbol,
                    &members[0],
                ) else {
                    continue;
                };
                let Some(receiver) = members[1..].iter().try_fold(root, |receiver, member| {
                    projections::field(program, receiver, member)
                }) else {
                    continue;
                };
                let endpoint = receiver.symbol;
                if call.receiver_symbol.is_valid() && call.receiver_symbol != endpoint {
                    return Err(vec![
                        Diagnostic::error(
                            "projected call receiver disagrees with its exact declared field",
                        )
                        .with_source_span(call.source_span),
                    ]);
                }
                let target = declared_receiver_target(
                    program,
                    receiver.type_reference,
                    call.target_symbol,
                    &call.target,
                    call.source_span,
                )?;
                updates.push((state.statement_nodes, index, endpoint, target));
            }
        }
    }
    for (statements, index, endpoint, target) in updates {
        let StatementNode::Call(call) =
            &mut program.statement_table.statements_mut(statements)[index]
        else {
            unreachable!("selected statement receiver changed shape")
        };
        call.receiver_symbol = endpoint;
        call.target_symbol = target;
    }
    for (expression, target) in expression_updates {
        let typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression_mut(expression)
        else {
            unreachable!("selected expression call changed shape")
        };
        call.target_symbol = target;
    }
    Ok(())
}

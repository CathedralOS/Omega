//! Preserve a query selection's erased region telescope through the same
//! static application custody used by other nested machine selections.
use super::{CallSite, Diagnostic, ExpressionNode, TypedTrees};

pub(super) fn retain(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut queries = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut expressions = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                validation::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
            }
            for expression in expressions {
                if let ExpressionNode::Call(call) = program.expression_table.expression(expression)
                    && let Some(query) = &call.native_code_layout
                {
                    queries.push((expression, machine.symbol, query.operand.clone()));
                }
            }
        }
    }
    for (expression, owner, operand) in queries {
        let application = super::applications::retain_authored_call(
            program,
            CallSite::Expression(expression),
            owner,
        );
        let lifetimes =
            super::application_lifetimes::bind(program, operand.symbol, &operand, application)?;
        if !program
            .expression_table
            .static_machine_application(application)
            .binding_declaration
            .is_valid()
        {
            let declaration = program
                .machines()
                .iter()
                .find(|machine| program.machine_or_state_symbol(machine, operand.symbol))
                .map(|machine| machine.symbol)
                .unwrap_or(operand.symbol);
            program
                .expression_table
                .static_machine_application_mut(application)
                .binding_declaration = declaration;
        }
        program
            .expression_table
            .set_static_machine_application_lifetimes(application, &lifetimes);
    }
    Ok(())
}

//! Close named machine operands under their applied invocation contract.
//!
//! Caller and provider telescopes are independent. A monomorphic expected
//! contract can determine the provider's complete tuple, but a generic expected
//! contract still requires universal refinement. Reuse ordinary instance
//! production and tuple admission; retain authored operands beside the selected
//! machine roster so checking sees the exact occurrence, not a globally changed
//! declaration. No contract is inferred from a provider's body.

mod invocations;
mod providers;
mod static_operands;

pub(super) use static_operands::names_any;

use super::{CallSite, Diagnostic, ExpressionNode, StatementNode, SymbolHandle, TypedTrees};

pub(super) fn close(program: &mut TypedTrees) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let invocations = invocations::collect(program);
    let mut changed = Vec::new();
    for invocation in invocations {
        // Capture source scope before a provider clone can retain its tuple.
        // Standalone specialization callers need the same custody as the
        // enclosing preparation pass, which normally already retained it.
        let application =
            super::applications::retain_authored_call(program, invocation.site, invocation.owner);
        let mut selected = invocation.machine_arguments.clone();
        let mut replaced = false;
        for (argument, expected) in selected.iter_mut().zip(&invocation.requirements) {
            let Some(entry) = providers::close(
                program,
                argument.symbol,
                expected,
                &invocation.types,
                application,
            )?
            else {
                continue;
            };
            argument.symbol = entry;
            replaced = true;
        }
        if !replaced {
            continue;
        }
        let authored = match invocation.site {
            CallSite::Expression(handle) => {
                let Some(call) = program.expression_table.call_node(handle) else {
                    continue;
                };
                call.machine_arguments.clone()
            }
            CallSite::Statement(handle) => {
                let Some(call) = program.statement_table.statement(handle).as_call() else {
                    continue;
                };
                call.machine_arguments.clone()
            }
        };
        program
            .expression_table
            .set_static_machine_application_selected_machines(application, &selected);
        let mut machine_ordinal = 0usize;
        let mut arguments = authored;
        for argument in &mut arguments {
            if invocations::is_machine(program, argument.symbol) {
                *argument = selected[machine_ordinal].clone();
                machine_ordinal += 1;
            }
        }
        match invocation.site {
            CallSite::Expression(handle) => {
                if let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle)
                {
                    call.authored_machine_application = application;
                    call.machine_arguments = arguments;
                }
            }
            CallSite::Statement(handle) => {
                if let StatementNode::Call(call) = program.statement_table.statement_mut(handle) {
                    call.authored_machine_application = application;
                    call.machine_arguments = arguments;
                }
            }
        }
        changed.push(invocation.owner);
    }
    Ok(changed)
}

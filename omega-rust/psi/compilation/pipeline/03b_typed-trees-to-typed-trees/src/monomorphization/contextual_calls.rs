//! Close named machine operands under their applied invocation contract.
//!
//! Caller and provider telescopes are independent. A monomorphic expected
//! contract can determine the provider's complete tuple, but a generic expected
//! contract still requires universal refinement. Reuse ordinary instance
//! production and tuple admission; retain authored operands beside the selected
//! machine roster so checking sees the exact occurrence, not a globally changed
//! declaration. No contract is inferred from a provider's body.

mod applications;
mod conformance_demands;
pub(super) use conformance_demands::prepare as prepare_conformance_machine_demands;
mod demands;
mod invocations;
mod providers;
mod static_operands;

pub(super) use static_operands::{is_open as operand_is_open, names_any};

use super::{CallSite, Diagnostic, ExpressionNode, StatementNode, SymbolHandle, TypedTrees};

pub(super) fn close(program: &mut TypedTrees) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let invocations = invocations::collect(program);
    let mut changed = Vec::new();
    for invocation in invocations {
        let member_demands = demands::members(program, invocation.target, &invocation.types)?;
        // Capture source scope before a provider clone can retain its tuple.
        // Standalone specialization callers need the same custody as the
        // enclosing preparation pass, which normally already retained it.
        let application =
            super::applications::retain_authored_call(program, invocation.site, invocation.owner);
        // Forwarded binders owe the same complete caller tuple as concrete
        // providers. Retain symbolic Source operands independently of whether
        // closing a provider rewrites an executable entry.
        let declaration = program
            .symbols
            .get(invocation.requirements[0].parameter)
            .parent;
        let retained = program
            .expression_table
            .static_machine_application_mut(application);
        if !retained.binding_declaration.is_valid() {
            retained.binding_declaration = declaration;
        }
        let mut bindings = program
            .expression_table
            .static_machine_application_bindings(application)
            .to_vec();
        for (parameter, actual) in &invocation.types {
            let value = if program
                .data_type_parameter_by_symbol(*parameter)
                .is_some_and(|parameter| parameter.kind.is_const_or_value())
            {
                typed_trees::static_applications::StaticMachineBindingValue::Const(*actual)
            } else {
                typed_trees::static_applications::StaticMachineBindingValue::Type(*actual)
            };
            if let Some(binding) = bindings
                .iter_mut()
                .find(|binding| binding.parameter == *parameter)
            {
                binding.value = value;
            } else {
                bindings.push(typed_trees::static_applications::StaticMachineBinding {
                    parameter: *parameter,
                    value,
                });
            }
        }
        program
            .expression_table
            .set_static_machine_application_bindings(application, &bindings);
        let mut selected = invocation.machine_arguments.clone();
        let mut replaced = false;
        for (ordinal, (argument, expected)) in selected
            .iter_mut()
            .zip(&invocation.requirements)
            .enumerate()
        {
            let closed_entry = providers::close(
                program,
                argument,
                expected.parameter,
                &expected.signature,
                &invocation.types,
                application,
                ordinal,
            )?;
            // A cache hit keeps its executable selection, but later typed
            // closure can update a member's demanded nominal application.
            // Refresh only its checking views; it is not another rewrite.
            demands::retain(
                program,
                application,
                ordinal,
                closed_entry.unwrap_or(argument.symbol),
                expected,
                &member_demands,
            )?;
            let Some(entry) = closed_entry else {
                continue;
            };
            argument.symbol = entry;
            let region_only_selection = program.machines().iter().any(|machine| {
                !machine.has_specialization_parameters()
                    && program
                        .machine_states(machine)
                        .iter()
                        .any(|state| state.symbol == entry)
            });
            if region_only_selection
                || program
                    .machine_specializations
                    .iter()
                    .any(|specialization| {
                        program
                            .machine_by_symbol(specialization.instance)
                            .is_some_and(|machine| {
                                program
                                    .machine_states(machine)
                                    .iter()
                                    .any(|state| state.symbol == entry)
                            })
                    })
            {
                // The authored recursive tuple stays in the retained source
                // application. The executable roster names its closed entry,
                // so outer specialization never keys a nested tuple by base
                // symbol alone. A region-only application already names its
                // original executable; erased regions are checking operands,
                // not another runtime specialization tuple.
                argument.application = None;
            }
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

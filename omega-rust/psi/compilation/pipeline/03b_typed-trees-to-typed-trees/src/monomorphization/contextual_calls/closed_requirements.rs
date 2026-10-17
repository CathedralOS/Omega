//! Derive closed requirement checking views from exact existing provider tuples.

use super::{CallSite, Diagnostic, TypedTrees, invocations};

/// Requirement closure substitutes the source demand after provider tuples
/// have settled. Derive its checking view from that same occurrence's retained
/// provider; this operation neither elects nor specializes another provider.
pub(in super::super) fn retain(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    for invocation in invocations::collect(program) {
        let (target, application) = match invocation.site {
            CallSite::Expression(handle) => {
                let Some(call) = program.expression_table.call_node(handle) else {
                    continue;
                };
                (call.target_symbol, call.authored_machine_application)
            }
            CallSite::Statement(handle) => {
                let Some(call) = program.statement_table.statement(handle).as_call() else {
                    continue;
                };
                (call.target_symbol, call.authored_machine_application)
            }
        };
        let Some(closed) = program.closed_requirement_application(target) else {
            continue;
        };
        if closed.template != invocation.target || !application.is_valid() {
            return Err(vec![Diagnostic::error(
                "closed requirement demand lost its exact source invocation",
            )]);
        }
        let (_, signature) = program
            .trait_machine_signature(closed.template)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "closed requirement demand lost its source declaration",
                )]
            })?;
        let source_demands = invocations::machine_demands(
            program,
            program.state_signature_type_parameters(signature),
        );
        if source_demands.len() != invocation.requirements.len() {
            return Err(vec![Diagnostic::error(
                "closed requirement demand changed its machine binder roster",
            )]);
        }
        for (ordinal, (argument, demand)) in invocation
            .machine_arguments
            .iter()
            .zip(&invocation.requirements)
            .enumerate()
        {
            let source = source_demands
                .get(ordinal)
                .filter(|source| source.parameter == demand.parameter)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "closed requirement demand lost its source machine binder",
                    )]
                })?;
            let retained = program
                .applied_machine_callable(
                    application,
                    ordinal,
                    argument.symbol,
                    source.parameter,
                    &source.signature,
                )
                .map(|view| view.provider);
            let Some(provider) = retained else {
                if program.has_applied_machine_selection(application, ordinal, argument.symbol) {
                    return Err(vec![Diagnostic::error(
                        "closed requirement call lost its determined source machine demand",
                    )]);
                }
                // Universal and forwarded selections retain ordinary callable
                // refinement; no determined provider tuple is manufactured.
                continue;
            };
            let bindings = program
                .expression_table
                .static_machine_application_bindings(provider);
            let lifetimes = program
                .expression_table
                .static_machine_application_lifetimes(provider);
            let caller_operands = &invocation.types;
            let caller_lifetimes = program
                .expression_table
                .static_machine_application_lifetimes(application)
                .to_vec();
            if !program.applied_machine_callable_matches(
                application,
                ordinal,
                argument.symbol,
                source.parameter,
                bindings,
                lifetimes,
                &source.signature,
                caller_operands,
                &caller_lifetimes,
            ) {
                return Err(vec![Diagnostic::error(
                    "closed requirement demand has a stale source provider tuple",
                )]);
            }
            if program.applied_machine_callable_matches(
                application,
                ordinal,
                argument.symbol,
                demand.parameter,
                bindings,
                lifetimes,
                &demand.signature,
                caller_operands,
                &caller_lifetimes,
            ) {
                continue;
            }
            program
                .prepare_applied_machine_callable(
                    application,
                    ordinal,
                    provider,
                    argument.symbol,
                    demand.parameter,
                    &demand.signature,
                    caller_operands,
                    &caller_lifetimes,
                )
                .map_err(|message| vec![Diagnostic::error(message)])?;
        }
    }
    Ok(())
}

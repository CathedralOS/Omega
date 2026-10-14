//! An uninstantiated declaration frontier cannot silently produce no loan.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{
    expression::{ExpressionHandle, StaticMachineArgument},
    statement::{StatementNode, TransitionTargetNode},
};

pub(super) fn check_calls(
    program: &dyn ::validation::CheckingInput,
    deferred: &[SymbolHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if deferred.is_empty() {
        return;
    }
    let mut expressions = Vec::new();
    for machine in program.machines() {
        // The producer retains authored templates beside their selected
        // instances. Only the latter can supply an executable call contract;
        // body cloning clears their specialization parameters. Source-body
        // checking still runs, but this closed-application fence belongs to
        // those instances, not to calls with deliberately unbound operands.
        if program.callable_body_has_open_parameters(machine) {
            continue;
        }
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Call(call) => {
                        check_target(
                            program,
                            machine,
                            state,
                            deferred,
                            call.target_symbol,
                            call.target.as_str(),
                            &call.machine_arguments,
                            program.statement_table.expression_handles(call.arguments),
                            diagnostics,
                        );
                    }
                    StatementNode::Transition(transition) => {
                        for target in transition.targets() {
                            if let TransitionTargetNode::Named {
                                path, arguments, ..
                            } = program.statement_table.transition_target(target)
                            {
                                let members =
                                    program.statement_table.name_path_members(path.members);
                                let name = members.last().map(|name| name.as_str()).unwrap_or("");
                                check_target(
                                    program,
                                    machine,
                                    state,
                                    deferred,
                                    path.symbol,
                                    name,
                                    &[],
                                    program.statement_table.expression_handles(*arguments),
                                    diagnostics,
                                );
                            }
                        }
                    }
                    _ => {}
                }
                expressions.clear();
                validation::expression_traversal::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                for handle in &expressions {
                    let Some(call) = program.expression_table.call_node(*handle) else {
                        continue;
                    };
                    check_target(
                        program,
                        machine,
                        state,
                        deferred,
                        call.target_symbol,
                        call.target.as_str(),
                        &call.machine_arguments,
                        program.expression_table.expression_handles(call.arguments),
                        diagnostics,
                    );
                }
            }
        }
    }
}

fn check_target(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    deferred: &[SymbolHandle],
    target: SymbolHandle,
    name: &str,
    static_arguments: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let unresolved_frontier = deferred.contains(&target)
        || program
            .machine_parameter_signature_in(machine, target)
            .is_some_and(|signature| deferred.contains(&signature.symbol))
        || {
            // Specialization may already have replaced a generic receiver call
            // with an exact machine. Its own result declaration is checked normally.
            let concrete = program.machines().iter().any(|candidate| {
                candidate.symbol == target
                    || typed_trees::state::state_by_symbol(
                        program.machine_states(candidate),
                        target,
                    )
                    .is_some()
            });
            !concrete
                && validation::named_conformance_target_requirement(program, machine, target)
                    .ok()
                    .flatten()
                    .is_some_and(|requirement| deferred.contains(&requirement.signature.symbol))
        };
    if unresolved_frontier
        && !closed_return_frontier(program, machine, state, target, static_arguments, arguments)
    {
        diagnostics.push(Diagnostic::error(format!(
            "call `{name}` in machine `{}` has a template-dependent returned-carrier lifetime frontier; a concrete checked callable is required before this call can produce or discard a value",
            machine.name,
        )));
    }
}

/// A template-dependent returned-carrier frontier admits a call once the
/// call's own static-callable substitution closes it: the instantiated
/// frontier must be complete and the substituted result-to-input relation
/// must resolve every returned view leaf to an exact caller source. A
/// closed view-free result is the same check with an empty frontier;
/// unresolved structure, unmatched leaves, and access escalation all keep
/// the historical rejection.
fn closed_return_frontier(
    program: &dyn ::validation::CheckingInput,
    caller: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    target: SymbolHandle,
    static_arguments: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
) -> bool {
    if let Ok(Some(requirement)) =
        validation::named_conformance_target_requirement(program, caller, target)
    {
        return complete_return_frontier(
            program,
            requirement.signature,
            &requirement.application.substitutions,
            &requirement.application.substitution_scope_starts,
            caller.symbol,
        );
    }
    let signature = program
        .traits()
        .iter()
        .flat_map(|definition| program.trait_machine_instances(definition))
        .find(|signature| signature.symbol == target)
        .or_else(|| program.machine_parameter_signature_in(caller, target));
    let Some(signature) = signature else {
        return false;
    };
    let Some(substitutions) = validation::closed_static_call_type_bindings(
        program,
        caller,
        state,
        signature,
        static_arguments,
        arguments,
    ) else {
        return false;
    };
    complete_return_frontier(program, signature, &substitutions, &[0], caller.symbol)
}

fn complete_return_frontier(
    program: &dyn ::validation::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    scope_starts: &[usize],
    argument_owner: SymbolHandle,
) -> bool {
    if !crate::borrow::view_link::substituted_result_frontier_is_complete(
        program,
        signature.return_type,
        substitutions,
        scope_starts,
    ) {
        return false;
    }
    !matches!(
        crate::borrow::view_link::resolve_applied_signature_view_return_source(
            program,
            signature,
            substitutions,
            scope_starts,
            argument_owner,
        ),
        crate::borrow::view_link::ViewReturnSource::Ambiguous(_)
    )
}

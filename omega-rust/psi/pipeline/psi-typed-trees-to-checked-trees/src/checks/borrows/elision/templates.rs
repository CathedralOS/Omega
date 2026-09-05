//! An uninstantiated declaration frontier cannot silently produce no loan.

use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument},
    statement::{StatementNode, TransitionTargetNode},
};

enum Receiver<'a> {
    Named(Option<&'a str>),
    Value(ExpressionHandle),
}

pub(super) fn check_calls(
    program: &TypedTrees,
    deferred: &[SymbolHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if deferred.is_empty() {
        return;
    }
    let mut expressions = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Call(call) => {
                        let receiver =
                            crate::lookup::statement_call_receiver_members(program, call)
                                .and_then(|members| members.last())
                                .map(|name| name.as_str());
                        check_target(
                            program,
                            machine,
                            state,
                            deferred,
                            call.target_symbol,
                            Receiver::Named(receiver),
                            call.target.as_str(),
                            &call.machine_arguments,
                            program.statement_table.expression_handles(call.arguments),
                            diagnostics,
                        );
                    }
                    StatementNode::Transition(transition) => {
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            if let TransitionTargetNode::Named {
                                path, arguments, ..
                            } = program.statement_table.transition_target(target)
                            {
                                let members =
                                    program.statement_table.name_path_members(path.members);
                                let receiver = members
                                    .len()
                                    .checked_sub(2)
                                    .and_then(|index| members.get(index));
                                let name = members.last().map(|name| name.as_str()).unwrap_or("");
                                check_target(
                                    program,
                                    machine,
                                    state,
                                    deferred,
                                    path.symbol,
                                    Receiver::Named(receiver.map(|name| name.as_str())),
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
                crate::monomorphization::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                for handle in &expressions {
                    let ExpressionNode::Call(call) = program.expression_table.expression(*handle)
                    else {
                        continue;
                    };
                    check_target(
                        program,
                        machine,
                        state,
                        deferred,
                        call.target_symbol,
                        Receiver::Value(call.receiver),
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
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    deferred: &[SymbolHandle],
    target: SymbolHandle,
    receiver: Receiver<'_>,
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
                    || program
                        .machine_states(candidate)
                        .iter()
                        .any(|state| state.symbol == target)
            });
            !concrete && {
                let requirement = match receiver {
                    Receiver::Named(Some(receiver)) => {
                        psi_validation::generic_bound_call_requirement(
                            program, machine, state, receiver, name,
                        )
                    }
                    Receiver::Named(None) => Ok(None),
                    Receiver::Value(receiver) => {
                        psi_validation::generic_bound_value_call_requirement(
                            program, machine, state, receiver, name,
                        )
                    }
                };
                requirement
                    .ok()
                    .flatten()
                    .is_some_and(|signature| deferred.contains(&signature.symbol))
            }
        };
    if unresolved_frontier
        && !closed_view_free_return(program, machine, state, target, static_arguments, arguments)
    {
        diagnostics.push(Diagnostic::error(format!(
            "call `{name}` in machine `{}` has a template-dependent returned-carrier lifetime frontier; a concrete checked callable is required before this call can produce or discard a value",
            machine.name,
        )));
    }
}

fn closed_view_free_return(
    program: &TypedTrees,
    caller: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    target: SymbolHandle,
    static_arguments: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
) -> bool {
    let signature = program
        .traits()
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition))
        .find(|signature| signature.symbol == target)
        .or_else(|| program.machine_parameter_signature_in(caller, target));
    let Some(signature) = signature else {
        return false;
    };
    let Some(substitutions) = psi_validation::closed_static_call_type_bindings(
        program,
        caller,
        state,
        signature,
        static_arguments,
        arguments,
    ) else {
        return false;
    };
    crate::borrow::view_link::substituted_result_is_view_free(
        program,
        signature.return_type,
        &substitutions,
    )
}

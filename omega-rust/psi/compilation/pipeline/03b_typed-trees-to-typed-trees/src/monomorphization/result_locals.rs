//! Retype compiler-inferred temporaries from their exact selected call result.
//!
//! A known static tuple need not be closed: forwarding caller binders already
//! determines the result's scope. Use the original selection for that result;
//! closedness controls executable specialization, not whether an inferred local
//! may retain foreign callee binders. Checking consumes the resulting type and
//! must not reconstruct the initializer to recover its lost substitutions.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn refresh_generic_call_results(
    program: &mut TypedTrees,
    candidates: &[super::Candidate],
    selections: &[super::CallSelection],
) -> Result<Vec<SymbolHandle>, Vec<diagnostics::Diagnostic>> {
    let mut updates = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (offset, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                let Some(local) = statement.as_local_data() else {
                    continue;
                };
                if !local.type_is_inferred {
                    continue;
                }
                let Some(selection) = selections.iter().find(|selection| {
                    selection.site == super::CallSite::Expression(local.initial_value)
                        && selection.caller_is_generic
                }) else {
                    continue;
                };
                if !selection.has_bound_arguments() {
                    continue;
                }
                let Some(callee) = crate::monomorphization::selection::state_by_symbol(
                    program,
                    selection.callee_symbol,
                ) else {
                    continue;
                };
                if callee.return_type.is_valid() {
                    updates.push((
                        state.statement_nodes,
                        offset,
                        local.type_reference,
                        callee.return_type,
                        selection.clone(),
                    ));
                }
            }
        }
    }
    let mut changed_owners = Vec::new();
    for (body, offset, previous_type, return_type, selection) in updates {
        let candidate =
            super::candidate_for_selection(&candidates[selection.candidate_index], &selection);
        let mut forwarded_symbols = Vec::new();
        for (ordinal, ((parameter, _, _), binding)) in candidate
            .template
            .const_parameters
            .iter()
            .zip(&candidate.const_bindings)
            .enumerate()
        {
            // A runtime-bound `Value` slot binds its declared carrier, not a
            // forwardable static subject; its realized parameter carries the
            // call's value into the result type through ordinary checking.
            if candidate.runtime_value_bindings[ordinal].is_some() {
                continue;
            }
            if let Some(binding) = binding
                && let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(*binding)
                && symbol.is_valid()
            {
                forwarded_symbols.push((*parameter, *symbol));
            }
        }
        let substitutions = candidate
            .template
            .type_parameters
            .iter()
            .zip(&candidate.type_bindings)
            .filter_map(|((symbol, _), binding)| binding.map(|binding| (*symbol, binding)))
            .chain(
                candidate
                    .template
                    .const_parameters
                    .iter()
                    .zip(&candidate.const_bindings)
                    .filter_map(|((symbol, _, _), binding)| {
                        binding.map(|binding| (*symbol, binding))
                    }),
            )
            .collect::<Vec<_>>();
        // The package-qualified identity traverses range expressions under
        // substitution; the ordinary display-oriented range identity does not.
        if program.package_qualified_type_identity(previous_type)
            == program.type_identity(TypeIdentityRequest {
                substitutions: &substitutions,
                ..TypeIdentityRequest::package_qualified(return_type)
            })
        {
            continue;
        }
        // Arena indices are one-based, so the copied result begins one past
        // the last pre-existing node; bounding substitution at count()+1 keeps
        // an already-materialized endpoint or binder sentinel out of the sweep.
        let type_start = program.type_reference_table.type_reference_count() + 1;
        let expression_start = program.expression_table.iter_expressions().count() + 1;
        // Copy the declaration result before substitution. An open caller's
        // result uses this call's exact constants or caller binders, never the
        // callee's unbound parameters or the inferred destination as evidence.
        let selected_return = crate::monomorphization::body_cloning::copy_type_reference(
            None,
            program,
            return_type,
            &forwarded_symbols,
        );
        let evidence_rewrites =
            super::body_rewriting::evidence_argument_rewrites(program, &candidate);
        let expressions = program
            .expression_table
            .iter_expressions()
            .filter(|(expression, _)| expression.arena_index() as usize >= expression_start)
            .map(|(expression, _)| expression)
            .collect::<Vec<_>>();
        for expression in expressions {
            super::body_rewriting::rewrite_index_algebra(program, expression, &evidence_rewrites);
        }
        // A runtime-bound `Value` slot cannot appear in an open caller's
        // inferred result: forwarded selections only bind static subjects, so
        // the runtime-expression root list stays empty here.
        super::const_values::substitute(program, &candidate, Some(expression_start), &[])
            .map_err(|error| vec![error])?;
        crate::monomorphization::body_rewriting::substitute_cloned_type_parameters(
            None, program, &candidate, type_start,
        )
        .map_err(|error| vec![error])?;
        if let StatementNode::LocalData(local) =
            &mut program.statement_table.statements_mut(body)[offset]
        {
            local.type_reference = selected_return;
            changed_owners.push(selection.lexical_owner);
        }
    }
    if !changed_owners.is_empty() {
        // Open result substitution is also a graph producer. Settle its owned
        // constraints before another selection reads the newly inferred type.
        let roots = typed_trees::references::DeclarationSelection::from_symbols(
            changed_owners.iter().copied(),
        );
        super::finalize_domain_instance_identities(program, &roots)
            .map_err(|diagnostic| vec![diagnostic])?;
    }
    Ok(changed_owners)
}

pub(super) fn refresh(program: &mut TypedTrees) -> Vec<SymbolHandle> {
    let selected_returns = program
        .machines()
        .iter()
        .filter(|machine| program.machine_type_parameters(machine).is_empty())
        .flat_map(|machine| program.machine_states(machine))
        .map(|state| (state.symbol, state.return_type))
        .collect::<Vec<_>>();
    let bodies = program
        .machines()
        .iter()
        .flat_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .map(move |state| (machine.symbol, state.statement_nodes))
        })
        .collect::<Vec<_>>();
    let mut updates = Vec::new();
    for (owner, body) in bodies {
        for (offset, statement) in program.statement_table.indexed_statements(body) {
            let Some(local) = statement.as_local_data() else {
                continue;
            };
            if !local.type_is_inferred {
                continue;
            }
            let Some(call) = program.expression_table.call_node(local.initial_value) else {
                continue;
            };
            if call.target_symbol.is_valid()
                && let Some((_, return_type)) =
                    selected_returns.iter().find(|(state, return_type)| {
                        *state == call.target_symbol && return_type.is_valid()
                    })
            {
                // Retain the initializer occurrence and its evaluation point.
                // Only its inferred destination follows the selected instance.
                updates.push((
                    owner,
                    body,
                    offset,
                    call.target_symbol,
                    program
                        .expression_table
                        .expression_handles(call.arguments)
                        .to_vec(),
                    *return_type,
                ));
            }
        }
    }
    let mut changed_owners = Vec::new();
    for (owner, body, offset, target, arguments, return_type) in updates {
        let return_type =
            substitute_runtime_bound_result_bounds(program, target, &arguments, return_type);
        if let StatementNode::LocalData(local) =
            &mut program.statement_table.statements_mut(body)[offset]
            && local.type_reference != return_type
        {
            local.type_reference = return_type;
            changed_owners.push(owner);
        }
    }
    changed_owners
}

/// A specialized callee's result range can name the realized trailing
/// parameter that carries a runtime-bound `Value` subject
/// (`-> u64[0..=Bound]`). Retyping the caller's inferred local with the raw
/// declaration would install a dead binder under a foreign scope: copy the
/// declared type and substitute each parameter-named leaf in a range endpoint
/// with the paired call argument, so the destination's index IS the captured
/// subject (`u64[0..=n]`). Return types whose endpoints name no parameter
/// keep the shared reference — ordinary calls allocate nothing.
fn substitute_runtime_bound_result_bounds(
    program: &mut TypedTrees,
    callee_state: SymbolHandle,
    arguments: &[ExpressionHandle],
    return_type: TypeReferenceHandle,
) -> TypeReferenceHandle {
    let Some(state) = crate::monomorphization::selection::state_by_symbol(program, callee_state)
    else {
        return return_type;
    };
    let substitutions = arguments
        .iter()
        .copied()
        .zip(
            program
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.symbol),
        )
        .map(|(argument, symbol)| (symbol, argument))
        .collect::<Vec<_>>();
    if !range_endpoints_name_parameters(program, return_type, &substitutions) {
        return return_type;
    }
    let copied =
        crate::monomorphization::body_cloning::copy_type_reference(None, program, return_type, &[]);
    substitute_named_endpoints(program, copied, &substitutions);
    copied
}

/// True when any range endpoint under `root` names one of the callee's state
/// parameters — the shape the runtime-bound specialization writes into its
/// signature.
fn range_endpoints_name_parameters(
    program: &dyn ::validation::CheckingInput,
    root: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, ExpressionHandle)],
) -> bool {
    crate::monomorphization::for_each_type_reference(
        program,
        &[root],
        |_, _| false,
        |program, minimum, maximum| {
            [minimum, maximum].into_iter().any(|endpoint| {
                let mut tree = Vec::new();
                validation::expression_traversal::collect_expression_tree(
                    program, endpoint, &mut tree,
                );
                tree.into_iter().any(|handle| {
                    matches!(
                        program.expression_table.expression(handle),
                        ExpressionNode::Name(path)
                            if substitutions
                                .iter()
                                .any(|(symbol, _)| *symbol == path.symbol)
                    )
                })
            })
        },
    )
}

/// Replace every name leaf inside the copied type's range endpoints that
/// names a callee parameter with the paired call argument's node payload. The
/// caller's argument subtree is shared read-only structure — reusing its
/// payload in the copied endpoint is the same reference reuse
/// `copy_expression` performs between arenas.
fn substitute_named_endpoints(
    program: &mut TypedTrees,
    root: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, ExpressionHandle)],
) {
    let endpoints = crate::monomorphization::type_reference_endpoints(program, root);
    for endpoint in endpoints
        .into_iter()
        .flat_map(|(minimum, maximum)| [minimum, maximum])
    {
        let mut tree = Vec::new();
        validation::expression_traversal::collect_expression_tree(program, endpoint, &mut tree);
        for member in tree {
            let replacement = match program.expression_table.expression(member) {
                ExpressionNode::Name(path) => substitutions
                    .iter()
                    .find(|(symbol, _)| *symbol == path.symbol)
                    .map(|(_, argument)| *argument),
                _ => None,
            };
            if let Some(argument) = replacement {
                let payload = program.expression_table.expression(argument).clone();
                *program.expression_table.expression_mut(member) = payload;
            }
        }
    }
}

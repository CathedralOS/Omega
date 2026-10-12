//! Settle occurrence-owned value results before their consumers select calls.
//!
//! Calls publish their actual result graph through the shared constructed-value
//! type arena. Inferred locals and nested argument projections consume that same
//! graph; a shared executable signature never supplies another caller's regions.
//!
//! A known static tuple need not be closed: forwarding caller binders already
//! determines the result's scope. Use the original selection for that result;
//! closedness controls executable specialization, not whether an inferred local
//! may retain foreign callee binders. Checking consumes the resulting type and
//! must not reconstruct the initializer to recover its lost substitutions.
//!
//! Inferred value locals consume the current shared result after compound
//! postorder publication, including updated ranges and projection types. No
//! source shape or old Unit sentinel decides freshness. Anonymous numeric
//! destinations retain their independently established landing; pattern place
//! markers carry proof bookkeeping and are not owned value snapshots.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;

mod derivations;
mod operand_captures;
mod reference_observations;
mod region_application;
pub(crate) use derivations::ResultDerivations;
pub(super) use reference_observations::materialize as materialize_reference_observations;

#[derive(Clone, Copy)]
struct InferredUpdate {
    owner: SymbolHandle,
    body: arena::HandleSpan<StatementNode>,
    offset: usize,
    type_reference: TypeReferenceHandle,
}

pub(super) fn refresh_generic_call_results(
    program: &mut TypedTrees,
    candidates: &[super::Candidate],
    selections: &[super::CallSelection],
    derivations: &mut ResultDerivations,
) -> Result<Vec<SymbolHandle>, Vec<diagnostics::Diagnostic>> {
    let updates = selections
        .iter()
        .filter_map(|selection| {
            let super::CallSite::Expression(expression) = selection.site else {
                return None;
            };
            if !selection.caller_is_generic || !selection.has_bound_arguments() {
                return None;
            }
            let callee = crate::monomorphization::selection::state_by_symbol(
                program,
                selection.callee_symbol,
            )?;
            callee.return_type.is_valid().then(|| {
                (
                    expression,
                    program
                        .expression_table
                        .constructed_value_type_reference(expression),
                    callee.return_type,
                    selection.clone(),
                )
            })
        })
        .collect::<Vec<_>>();
    let mut changed_owners = Vec::new();
    for (expression, previous_type, return_type, selection) in updates {
        let candidate = super::candidate_for_selection(
            program,
            &candidates[selection.candidate_index],
            &selection,
        );
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
                && let Ok(argument) = program.static_operand_from_type_reference(*binding)
                && argument.symbol.is_valid()
            {
                forwarded_symbols.push((*parameter, argument.symbol));
            }
        }
        let lifetimes = program
            .expression_table
            .static_machine_application_lifetimes(candidate.binding_application)
            .iter()
            .map(|binding| (binding.parameter.clone(), binding.argument.clone()))
            .collect::<Vec<_>>();
        let original_call = program
            .expression_table
            .source_call_node(expression)
            .ok_or_else(|| {
                vec![diagnostics::Diagnostic::error(
                    "generic result derivation lost its original call occurrence",
                )]
            })?;
        let original_arguments = program
            .expression_table
            .expression_handles(original_call.arguments)
            .to_vec();
        let bindings = super::applications::candidate_bindings(&candidate);
        let input_lifetimes = lifetimes
            .iter()
            .map(|(parameter, argument)| (parameter.clone(), argument.named().cloned()))
            .collect::<Vec<_>>();
        let carriers = candidate
            .template
            .const_parameters
            .iter()
            .map(|(_, _, carrier)| *carrier)
            .collect::<Vec<_>>();
        let inputs = derivations::inputs(
            program,
            selection.lexical_owner,
            selection.callee_symbol,
            return_type,
            &original_arguments,
            &input_lifetimes,
            candidate.binding_application,
            &bindings,
            selected_result_parameters(program, selection.callee_symbol).unwrap_or_default(),
            &carriers,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let retained = derivations.result(program, expression, true, &inputs);
        let selected_return = if retained.is_valid() {
            retained
        } else {
            // Arena indices are one-based, so the copied result begins one past
            // the last pre-existing node; bounding substitution at count()+1 keeps
            // an already-materialized endpoint or binder sentinel out of the sweep.
            let type_start = program.type_reference_table.type_reference_count() + 1;
            let expression_start = program.expression_table.expression_count() + 1;
            // Copy the declaration result before substitution. An open caller's
            // result uses this call's exact constants or caller binders, never the
            // callee's unbound parameters or the inferred destination as evidence.
            let selected_return = program
                .apply_callable_type(return_type, &[], &lifetimes, selection.callee_symbol)
                .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?;
            // Application already owns a fresh graph. Remap that graph in place:
            // copying it again leaves an unused open graph inside the substitution
            // interval, where its callee binders would be mistaken for value uses.
            let tables = &mut program.tables;
            tables.type_reference_table.remap_symbols_in(
                selected_return,
                &mut tables.expression_table,
                &forwarded_symbols,
            );
            let application_bindings = super::applications::candidate_bindings(&candidate);
            let evidence_rewrites = super::application_evidence::evidence_argument_rewrites(
                program,
                &application_bindings,
            );
            let static_rewrites = super::body_rewriting::forwarded_static_argument_rewrites(
                program,
                &application_bindings,
            );
            let expressions = program
                .expression_table
                .iter_expressions()
                .filter(|(expression, _)| expression.arena_index() as usize >= expression_start)
                .map(|(expression, _)| expression)
                .collect::<Vec<_>>();
            for expression in expressions {
                super::application_evidence::rewrite_index_algebra(
                    program,
                    expression,
                    &evidence_rewrites,
                    &static_rewrites,
                );
            }
            // A runtime-bound `Value` slot cannot appear in an open caller's
            // inferred result: forwarded selections only bind static subjects, so
            // the runtime-expression root list stays empty here.
            super::const_values::substitute(None, program, &candidate, Some(expression_start), &[])
                .map_err(|error| vec![error])?;
            crate::monomorphization::body_rewriting::substitute_cloned_type_parameters(
                None, program, &candidate, type_start,
            )
            .map_err(|error| vec![error])?;
            selected_return
        };
        derivations.retain(expression, true, inputs, selected_return);
        if previous_type == selected_return
            || (program
                .type_reference_table
                .contains_type_reference(previous_type)
                && same_result_application(
                    program,
                    selection.lexical_owner,
                    previous_type,
                    selected_return,
                ))
        {
            continue;
        }
        program
            .expression_table
            .set_constructed_value_type_reference(expression, selected_return);
        changed_owners.push(selection.lexical_owner);
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

/// Publish one ordinary call result in the shared child-before-parent walk.
/// None means the separate generic selection producer owns this open call;
/// Some(zero) means this ordinary result is currently unavailable.
pub(crate) fn retain(
    program: &mut TypedTrees,
    owner: SymbolHandle,
    state_symbol: SymbolHandle,
    expression: ExpressionHandle,
    derivations: &mut ResultDerivations,
) -> Result<Option<TypeReferenceHandle>, Vec<diagnostics::Diagnostic>> {
    let Some(executed) = program.expression_table.call_node(expression).cloned() else {
        return Ok(Some(TypeReferenceHandle::invalid()));
    };
    let Some(original) = program.expression_table.source_call(&executed).cloned() else {
        return Err(vec![diagnostics::Diagnostic::error(
            "ordinary result derivation lost its original call occurrence",
        )]);
    };
    let public = program
        .expression_table
        .static_machine_application_is_valid(original.authored_machine_application)
        .then(|| {
            program
                .expression_table
                .static_machine_application(original.authored_machine_application)
                .public_result_type
        })
        .filter(|reference| reference.is_valid());
    let selected = if public.is_some() {
        public
    } else if let Some((machine, state)) = validation::find_state(program, executed.target_symbol)
        .or_else(|| validation::find_machine_head(program, executed.target_symbol))
    {
        if machine.has_specialization_parameters() {
            return Ok(None);
        }
        Some(state.return_type)
    } else {
        program
            .machine_parameter_signature(executed.target_symbol)
            .map(|(_, signature)| signature.return_type)
    };
    let Some(return_type) = selected.filter(|reference| reference.is_valid()) else {
        return Ok(Some(TypeReferenceHandle::invalid()));
    };
    let Some(machine) = program.machine_by_symbol(owner) else {
        return Ok(Some(TypeReferenceHandle::invalid()));
    };
    let Some(state) = program.state_by_symbol(state_symbol) else {
        return Ok(Some(TypeReferenceHandle::invalid()));
    };
    let mut call = executed.clone();
    call.arguments = original.arguments;
    call.authored_machine_application = original.authored_machine_application;
    let (target, lifetimes) = if public.is_some() {
        (SymbolHandle::invalid(), Vec::new())
    } else {
        (
            executed.target_symbol,
            region_application::bindings(program, machine, state, &call)?,
        )
    };
    let arguments = program
        .expression_table
        .expression_handles(original.arguments)
        .to_vec();
    let inputs = derivations::inputs(
        program,
        owner,
        target,
        return_type,
        &arguments,
        &lifetimes,
        original.authored_machine_application,
        &[],
        selected_result_parameters(program, target).unwrap_or_default(),
        &[],
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let retained = derivations.result(program, expression, false, &inputs);
    let result = if retained.is_valid() {
        retained
    } else {
        apply_result_type(program, target, &arguments, return_type, &lifetimes)?
    };
    derivations.retain(expression, false, inputs, result);
    let previous = program
        .expression_table
        .constructed_value_type_reference(expression);
    if previous == result
        || (program
            .type_reference_table
            .contains_type_reference(previous)
            && same_result_application(program, owner, previous, result))
    {
        Ok(Some(previous))
    } else {
        Ok(Some(result))
    }
}

/// Consume the current shared result after calls and compound expressions have
/// published this round. Authored destinations and pattern place markers remain
/// independent. Process declarations in order so later aliases see fresh types.
pub(super) fn refresh_inferred_local_results(
    program: &mut TypedTrees,
    unavailable: &[crate::constructed_results::UnavailableValueType],
) -> Result<Vec<SymbolHandle>, Vec<diagnostics::Diagnostic>> {
    let mut sites = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (offset, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                if let Some(local) = statement.as_local_data()
                    && local.type_is_inferred
                    && local.role == language_core::LocalBindingRole::Value
                {
                    sites.push((
                        machine.symbol,
                        state.symbol,
                        state.statement_nodes,
                        offset,
                        local.initial_value,
                    ));
                }
            }
        }
    }
    // Resolve all raw capture operands before contextual adaptation. A scalar
    // index observes the reference a call produced; it does not change the
    // shared call result graph or the callee's declared result. Preserve the
    // incoming capture operation while ordinary aliases still settle below in
    // declaration order from the current occurrence-owned expression graph.
    let mut captures = sites
        .iter()
        .filter_map(|(owner, state_symbol, body, offset, initial_value)| {
            let (machine, state) = program.machine_state(*owner, *state_symbol)?;
            let local = program.statement_table.statements(*body)[*offset].as_local_data()?;
            if !local.is_operand_capture {
                return None;
            }
            let type_reference = validation::expression_result_type_reference(
                program,
                machine,
                state,
                *initial_value,
            )?;
            Some(InferredUpdate {
                owner: *owner,
                body: *body,
                offset: *offset,
                type_reference,
            })
        })
        .collect::<Vec<_>>();
    operand_captures::adapt(program, &mut captures);
    let mut changed_owners = Vec::new();
    for (owner, state_symbol, body, offset, initial_value) in sites {
        let Some((machine, state)) = program.machine_state(owner, state_symbol) else {
            continue;
        };
        if let Some(refusal) = unavailable
            .iter()
            .find(|refusal| refusal.expression == initial_value)
        {
            return Err(refusal.diagnostics.clone());
        }
        let result =
            validation::expression_result_type_reference(program, machine, state, initial_value);
        // Anonymous numeric expressions acquire their actual carrier through
        // destination landing. An absent expression result cannot invent one.
        if result.is_none() && validation::has_anonymous_numeric_results(program, initial_value) {
            continue;
        }
        let result = result.unwrap_or_else(TypeReferenceHandle::invalid);
        let Some(local) = program.statement_table.statements(body)[offset].as_local_data() else {
            continue;
        };
        let result = if local.is_operand_capture {
            captures
                .iter()
                .find(|capture| capture.body == body && capture.offset == offset)
                .map_or(result, |capture| capture.type_reference)
        } else {
            result
        };
        if local.type_reference == result
            || (local.type_reference.is_valid()
                && result.is_valid()
                && same_result_application(program, owner, local.type_reference, result))
        {
            continue;
        }
        if let StatementNode::LocalData(local) =
            &mut program.statement_table.statements_mut(body)[offset]
        {
            local.type_reference = result;
            changed_owners.push(owner);
        }
    }
    Ok(changed_owners)
}

/// Result storage belongs to the caller's occurrence, never the callee's
/// declaration telescope. Inference and explicit reference observations share
/// this region and runtime-bound application before installing a local type.
fn apply_result_type(
    program: &mut TypedTrees,
    target: SymbolHandle,
    arguments: &[ExpressionHandle],
    return_type: TypeReferenceHandle,
    lifetimes: &[(
        typed_trees::name::Identifier,
        Option<typed_trees::name::Identifier>,
    )],
) -> Result<TypeReferenceHandle, Vec<diagnostics::Diagnostic>> {
    let return_type = if lifetimes.is_empty() {
        return_type
    } else {
        program
            .apply_callable_result_type(return_type, lifetimes, target)
            .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?
    };
    Ok(substitute_runtime_bound_result_bounds(
        program,
        target,
        arguments,
        return_type,
    ))
}

fn same_result_application(
    program: &TypedTrees,
    owner: SymbolHandle,
    previous: TypeReferenceHandle,
    current: TypeReferenceHandle,
) -> bool {
    typed_trees::type_identity::applied_type_identity(
        program,
        previous,
        typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(owner),
        &[],
        typed_trees::type_identity::TypeIdentityQualification::Ordinary,
    )
    .zip(typed_trees::type_identity::applied_type_identity(
        program,
        current,
        typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(owner),
        &[],
        typed_trees::type_identity::TypeIdentityQualification::Ordinary,
    ))
    .is_some_and(|(previous, current)| previous == current)
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
    let Some(parameters) = selected_result_parameters(program, callee_state) else {
        return return_type;
    };
    let substitutions = arguments
        .iter()
        .copied()
        .zip(parameters.iter().map(|parameter| parameter.symbol))
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

/// The same selected formal correspondence feeds result substitution and
/// producer-input observation. It is ordered declaration data, not another
/// provider or same-spelled callable selection.
fn selected_result_parameters(
    program: &TypedTrees,
    callee_state: SymbolHandle,
) -> Option<&[typed_trees::signature::StateParameter]> {
    if let Some(state) = crate::monomorphization::selection::state_by_symbol(program, callee_state)
    {
        Some(program.state_parameters(state))
    } else if let Some(signature) = program
        .trait_machine_signature(callee_state)
        .map(|(_, signature)| signature)
        .or_else(|| {
            program
                .machine_parameter_signature(callee_state)
                .map(|(_, signature)| signature)
        })
    {
        // The selected result already belongs to this exact signature. Its
        // fresh runtime formals still need the initializer's actual operands,
        // independently of whether the callable owns a body.
        Some(program.state_signature_parameters(signature))
    } else {
        None
    }
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

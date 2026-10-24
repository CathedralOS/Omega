//! Settle compiler-inferred temporaries before their consumers select calls.
//!
//! A known static tuple need not be closed: forwarding caller binders already
//! determines the result's scope. Use the original selection for that result;
//! closedness controls executable specialization, not whether an inferred local
//! may retain foreign callee binders. Checking consumes the resulting type and
//! must not reconstruct the initializer to recover its lost substitutions.
//!
//! Resolved-tree normalization also captures computations before their result
//! type is available. A remaining Unit sentinel is not the computation's type:
//! read its existing result through the shared expression-type owner. Otherwise
//! a captured cast, for example, hides its explicit carrier from operator
//! selection. Preserve already-derived ranges and projection types; only call
//! results must follow subsequent specialization. Pattern place markers carry
//! proof bookkeeping, not owned snapshots, and retain their sentinel.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::statement::StatementNode;
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::TypeReferenceHandle;

mod dispatch_subjects;
mod operand_captures;
mod reference_observations;
pub(super) use dispatch_subjects::materialize as materialize_dispatch_subjects;
pub(crate) use dispatch_subjects::require_complete as require_complete_dispatch_subjects;
pub(super) use reference_observations::materialize as materialize_reference_observations;

#[derive(Clone, Copy)]
struct InferredUpdate {
    owner: SymbolHandle,
    body: arena::HandleSpan<StatementNode>,
    offset: usize,
    type_reference: TypeReferenceHandle,
}

/// Commit inferred results only after operand contexts agree on their stored
/// values. Raw call results (including applied regions and bounds) participate
/// together, so one adapted range endpoint cannot alter the other's selection.
fn apply_updates(program: &mut TypedTrees, mut updates: Vec<InferredUpdate>) -> Vec<SymbolHandle> {
    operand_captures::adapt(program, &mut updates);
    let mut changed_owners = Vec::new();
    for update in updates {
        let Some(local) =
            program.statement_table.statements(update.body)[update.offset].as_local_data()
        else {
            continue;
        };
        if local.type_reference == update.type_reference
            || typed_trees::type_identity::applied_type_identity(
                program,
                local.type_reference,
                update.owner,
                &[],
            )
            .zip(typed_trees::type_identity::applied_type_identity(
                program,
                update.type_reference,
                update.owner,
                &[],
            ))
            .is_some_and(|(previous, current)| previous == current)
        {
            continue;
        }
        if let StatementNode::LocalData(local) =
            &mut program.statement_table.statements_mut(update.body)[update.offset]
        {
            local.type_reference = update.type_reference;
            changed_owners.push(update.owner);
        }
    }
    changed_owners
}

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
                if program
                    .expression_table
                    .call_node(local.initial_value)
                    .is_some_and(|call| call.native_code_layout.is_some())
                {
                    continue;
                }
                if !local.type_is_inferred || !local.role.is_value_binding() {
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
                        local.is_operand_capture,
                        callee.return_type,
                        selection.clone(),
                    ));
                }
            }
        }
    }
    let mut inferred_updates = Vec::new();
    for (body, offset, previous_type, is_operand_capture, return_type, selection) in updates {
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
        if !is_operand_capture
            && program.package_qualified_type_identity(previous_type)
                == program.type_identity(TypeIdentityRequest {
                    substitutions: &substitutions,
                    ..TypeIdentityRequest::package_qualified(return_type)
                })
        {
            continue;
        }
        // Arena indices are one-based, so the copied result begins one past
        // the last pre-existing node; using the backing arena count keeps retained source
        // predicates out of the sweep even when iteration roots were projected.
        let type_start = program.type_reference_table.type_reference_count() + 1;
        let expression_start = program.expression_table.expression_count() + 1;
        let lifetimes = program
            .expression_table
            .static_machine_application_lifetimes(candidate.binding_application)
            .iter()
            .map(|binding| (binding.parameter.clone(), binding.argument.clone()))
            .collect::<Vec<_>>();
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
        let evidence_rewrites =
            super::body_rewriting::evidence_argument_rewrites(program, &candidate);
        let static_rewrites =
            super::body_rewriting::forwarded_static_argument_rewrites(program, &candidate);
        let expressions = program
            .expression_table
            .iter_expressions()
            .filter(|(expression, _)| expression.arena_index() as usize >= expression_start)
            .map(|(expression, _)| expression)
            .collect::<Vec<_>>();
        for expression in expressions {
            super::body_rewriting::rewrite_index_algebra(
                program,
                expression,
                &evidence_rewrites,
                &static_rewrites,
            );
        }
        // A runtime-bound `Value` slot cannot appear in an open caller's
        // inferred result: forwarded selections only bind static subjects, so
        // the runtime-expression root list stays empty here.
        super::const_values::substitute(program, &candidate, Some(expression_start), &[])
            .map_err(|error| vec![error])?;
        crate::monomorphization::body_rewriting::substitute_cloned_type_parameters(
            None,
            program,
            &candidate,
            type_start,
            SymbolHandle::invalid(),
        )
        .map_err(|error| vec![error])?;
        inferred_updates.push(InferredUpdate {
            owner: selection.lexical_owner,
            body,
            offset,
            type_reference: selected_return,
        });
    }
    let changed_owners = apply_updates(program, inferred_updates);
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

pub(crate) fn refresh(
    program: &mut TypedTrees,
    unavailable: &[crate::constructed_results::UnavailableValueType],
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
                if !local.type_is_inferred || !local.role.is_value_binding() {
                    continue;
                }
                let is_call = program
                    .expression_table
                    .call_node(local.initial_value)
                    .is_some();
                let needs_inference = matches!(
                    program
                        .type_reference_table
                        .type_reference(local.type_reference),
                    typed_trees::types::TypeReferenceNode::Unit
                );
                if !is_call
                    && !local.is_operand_capture
                    && !needs_inference
                    && local.role != language_core::LocalBindingRole::DispatchSubject
                {
                    continue;
                }
                let reference = if is_call {
                    if let Some(unavailable) = unavailable
                        .iter()
                        .find(|unavailable| unavailable.expression == local.initial_value)
                    {
                        return Err(unavailable.diagnostics.clone());
                    }
                    let retained = program
                        .expression_table
                        .constructed_value_type_reference(local.initial_value);
                    retained.is_valid().then_some(retained)
                } else {
                    validation::expression_result_type_reference(
                        program,
                        machine,
                        state,
                        local.initial_value,
                    )
                };
                let Some(type_reference) = reference else {
                    continue;
                };
                // Generated operand captures follow their completed value on
                // every pass. Their former provisional carrier or qualifiers
                // are not evidence about a newly selected operator result.
                // The ordinary consumer adaptation in apply_updates rebuilds
                // legitimate scalar-reference reads from these raw results.
                if !is_call
                    && !local.is_operand_capture
                    && !needs_inference
                    && local.role != language_core::LocalBindingRole::DispatchSubject
                    && validation::reference_shell(program, type_reference).is_none()
                {
                    continue;
                }
                updates.push(InferredUpdate {
                    owner: machine.symbol,
                    body: state.statement_nodes,
                    offset,
                    type_reference,
                });
            }
        }
    }
    Ok(apply_updates(program, updates))
}

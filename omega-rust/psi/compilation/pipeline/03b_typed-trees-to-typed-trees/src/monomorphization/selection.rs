//! Discover complete call tuples without admitting their authored bounds.
//!
//! This file collects call proposals and call selections.
//! `callee_proposals.rs` resolves callees and their machine proposals,
//! `proposals.rs` owns the shared proposal ledger and binding context,
//! `selection_assembly.rs` assembles selections from proposals,
//! `static_bindings.rs` infers static, fixed-array and domain bindings.

use validation::collect_expression_tree;
use validation::collect_statement_expression_trees;

mod callee_proposals;
mod proposals;
mod selection_assembly;
mod static_bindings;

pub(crate) use callee_proposals::{
    ContractExpressionScope, collect_machine_proposals_for_callee, contract_expression_handles,
    contract_expression_scopes, resolve_callee, state_by_symbol,
};
pub(crate) use proposals::{BindingContext, StaticProposals};
pub(crate) use selection_assembly::{
    proposal_bindings, selection_for_call, selection_from_proposals, unique_complete_selections,
    upsert_selection,
};
pub(crate) use static_bindings::{infer_static_bindings, names_type_binder};

use super::{
    CallSite, ExpressionHandle, ExpressionNode, StatementNode, StaticMachineArgument, SymbolHandle,
    TypeReferenceHandle,
};
use crate::monomorphization::{CallSelection, CalleeState, Candidate, const_arguments};
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ExplicitArgumentCapacity {
    WithinBounds,
    Exceeded,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_call_proposals(
    program: &dyn ::validation::CheckingInput,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    candidates: &[Candidate],
    callee_states: &[CalleeState],
    site: CallSite,
    target_symbol: SymbolHandle,
    machine_arguments: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
    expected_return: Option<TypeReferenceHandle>,
    scope_limit: usize,
    proposals: &mut StaticProposals,
) -> ExplicitArgumentCapacity {
    let Some(callee) = resolve_callee(callee_states, target_symbol) else {
        return ExplicitArgumentCapacity::WithinBounds;
    };
    let explicit_arguments = collect_machine_proposals_for_callee(
        program,
        candidates,
        callee,
        machine_arguments,
        Some((caller_state, scope_limit)),
        proposals,
    );
    if explicit_arguments.is_err() {
        return ExplicitArgumentCapacity::Exceeded;
    }
    // Authored type selections are fixed before argument compatibility. An
    // argument with a wider declared range must not reselect that type; its
    // value still owes the selected range at the ordinary call check. Keep
    // compatibility proposals separate so their repeated occurrences must
    // still agree, rather than treating the first inference as explicit.
    let explicit_type_count = proposals.types.len();
    let mut inferred = StaticProposals::default();

    // An explicit bound is selected before compatibility. It must not conflict
    // with an ordinary argument's narrower declared endpoint.
    let fixed_range_parameters: Vec<_> = machine_arguments
        .iter()
        .filter(|argument| {
            const_arguments::spelling(program, argument).is_some()
                || const_arguments::forwarded_type(program, argument).is_valid()
                || const_arguments::is_runtime_value_subject(program, argument)
        })
        .enumerate()
        .map(|(parameter_index, _)| parameter_index)
        .collect();
    let candidate = &candidates[callee.candidate_index];
    let context = BindingContext::new(
        program,
        &candidate.template.type_parameters,
        &candidate.template.const_parameters,
        Some(&fixed_range_parameters),
        callee.candidate_index,
    );
    for (position, (argument, required)) in arguments
        .iter()
        .zip(callee.parameter_types.iter())
        .enumerate()
    {
        // Calls bind argument values, not the storage those values observe.
        // Place lookup strips a borrow to its referent and can infer T instead
        // of &T. The shared value query also covers declaration-backed leaves;
        // unresolved values supply no proposal, never an underlying-place guess.
        let Some(actual) = validation::expression_result_type_reference(
            program,
            caller_machine,
            caller_state,
            *argument,
        ) else {
            continue;
        };
        if !program
            .typed_program()
            .callable_body_has_open_parameters(caller_machine)
            && static_bindings::names_type_binder(program, actual)
        {
            continue;
        }
        let first = inferred.constants.len();
        infer_static_bindings(context, *required, actual, &mut inferred);
        let descriptors = program.data_application_value_fields(actual);
        for &(_, binder, binding) in &inferred.constants[first..] {
            if !candidate.template.value_const_parameters.contains(&binder)
                || program
                    .static_const_literal_from_type_reference(binding)
                    .is_some()
            {
                continue;
            }
            if let Some((_, field, _)) = descriptors.iter().find(|(_, _, index)| *index == binding)
            {
                inferred.runtime_values.push((
                    callee.candidate_index,
                    binder,
                    crate::monomorphization::RuntimeValueBinding::CapturedArgument {
                        site,
                        position: u32::try_from(position).expect("argument ordinal"),
                        source: *argument,
                        descriptor: field.symbol,
                    },
                ));
            }
        }
    }
    if let Some(actual) = expected_return {
        // Result context can fill an omitted endpoint, but cannot reselect a
        // bound already supplied by an argument (even a still-open one).
        // The destination's range is a later compatibility obligation.
        let mut fixed_result_parameters = fixed_range_parameters;
        // Compatibility inference above already bound this candidate's const
        // slots; the result context reads them as fixed the same way.
        fixed_result_parameters.extend(
            proposals
                .constants
                .iter()
                .chain(inferred.constants.iter())
                .filter_map(|(candidate_index, parameter_index, _)| {
                    (*candidate_index == callee.candidate_index).then_some(*parameter_index)
                }),
        );
        let result_context = BindingContext::new(
            program,
            &candidate.template.type_parameters,
            &candidate.template.const_parameters,
            Some(&fixed_result_parameters),
            callee.candidate_index,
        );
        infer_static_bindings(result_context, callee.return_type, actual, &mut inferred);
    }
    // Compatibility proposals merge only where no explicit proposal already
    // names that binder; an authored selection stays fixed. Const proposals
    // keep the order they were derived in, after the explicit ones.
    for proposal in inferred.types {
        if !proposals.types[..explicit_type_count]
            .iter()
            .any(|explicit| explicit.0 == proposal.0 && explicit.1 == proposal.1)
        {
            proposals.types.push(proposal);
        }
    }
    proposals.constants.extend(inferred.constants);
    for proposal in inferred.runtime_values {
        if !proposals
            .runtime_values
            .iter()
            .any(|explicit| explicit.0 == proposal.0 && explicit.1 == proposal.1)
        {
            proposals.runtime_values.push(proposal);
        }
    }
    ExplicitArgumentCapacity::WithinBounds
}

/// Complete logical applications share ordinary proposal and binding
/// derivation. They are returned separately: retaining their source tuples is
/// not an executable instance demand. Declaration predicates see no body locals.
pub(super) fn collect_contract_call_selections(
    program: &dyn ::validation::CheckingInput,
    scopes: &[ContractExpressionScope<'_>],
    candidates: &[Candidate],
    callee_states: &[CalleeState],
) -> Vec<CallSelection> {
    let mut selections = Vec::new();
    for occurrences in scopes.chunk_by(|left, right| left.expression == right.expression) {
        let [scope] = occurrences else {
            // One shared expression cannot acquire a tuple from another
            // declaration telescope. Copies must retain their own occurrence.
            continue;
        };
        let Some(state) = scope.state else {
            // Bodyless declarations still contribute executable exclusion,
            // but provide no state telescope for this ordinary call proposal.
            continue;
        };
        let Some(call) = program.expression_table.call_node(scope.expression) else {
            continue;
        };
        if typed_trees::operator::resolve_named_expression_call(program, call).is_some() {
            continue;
        }
        if let Some(selection) = selection_assembly::selection_for_call_with_local_prefix(
            program,
            scope.machine,
            state,
            candidates,
            callee_states,
            CallSite::Expression(scope.expression),
            call.target_symbol,
            &call.machine_arguments,
            program.expression_table.expression_handles(call.arguments),
            None,
            program
                .typed_program()
                .callable_body_has_open_parameters(scope.machine),
            0,
        ) && selection.is_complete()
            && !const_arguments::has_pending_type_arguments(program, &selection.type_bindings)
        {
            upsert_selection(&mut selections, selection);
        }
    }
    selections
}

pub(super) fn collect_call_selections(
    program: &dyn ::validation::CheckingInput,
    candidates: &[Candidate],
    callee_states: &[CalleeState],
    contract_expressions: &[ExpressionHandle],
) -> Vec<CallSelection> {
    let mut selections = Vec::new();
    // Membership lists only answer `contains` inside per-expression loops;
    // keep them as sets so each query is O(1) rather than a rescan.
    let contract_expressions: HashSet<ExpressionHandle> =
        contract_expressions.iter().copied().collect();
    let mut covered_expressions = HashSet::new();
    let expected_calls = validation::collect_expected_expression_calls(program);

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for handle in state.statement_nodes.handles() {
                match program.statement_table.statement(handle) {
                    StatementNode::Call(call)
                        if typed_trees::operator::declaration_by_symbol(
                            program,
                            call.target_symbol,
                        )
                        .is_none() =>
                    {
                        if let Some(selection) = selection_for_call(
                            program,
                            machine,
                            state,
                            candidates,
                            callee_states,
                            CallSite::Statement(handle),
                            call.target_symbol,
                            &call.machine_arguments,
                            program.statement_table.expression_handles(call.arguments),
                            None,
                            program
                                .typed_program()
                                .callable_body_has_open_parameters(machine),
                        ) {
                            upsert_selection(&mut selections, selection);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Every expression call retains its lexical caller and the shared receiving
    // context. A local initializer, indexed assignment and match arm do not
    // require different generic-selection paths.
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut expressions = Vec::new();
            // Calls in declaration types have the same lexical owner as body
            // calls. Their landed argument types can infer ordinary binders;
            // the arena fallback deliberately has no such caller context.
            let mut visited_types = Vec::new();
            for reference in std::iter::once(state.return_type)
                .chain(
                    program
                        .state_parameters(state)
                        .iter()
                        .map(|parameter| parameter.type_reference),
                )
                .chain(
                    program
                        .machine_owned_data(machine)
                        .iter()
                        .map(|owned| owned.type_reference),
                )
            {
                const_arguments::collect_type_expressions(
                    program,
                    reference,
                    &mut visited_types,
                    &mut expressions,
                );
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let Some(local) = statement.as_local_data() {
                    const_arguments::collect_type_expressions(
                        program,
                        local.type_reference,
                        &mut visited_types,
                        &mut expressions,
                    );
                }
                if !matches!(statement, StatementNode::AssemblyFact(_)) {
                    collect_statement_expression_trees(program, statement, &mut expressions);
                }
            }
            // Erased proof-output calls retain the same lexical owner as
            // ordinary calls. The arena fallback cannot recover its binders.
            for call in &program.proof_output_calls {
                if call.machine_symbol == machine.symbol && call.state_symbol == state.symbol {
                    collect_expression_tree(program, call.call, &mut expressions);
                }
            }
            for expression in expressions {
                if covered_expressions.contains(&expression)
                    || contract_expressions.contains(&expression)
                {
                    continue;
                }

                let Some(call) = program.expression_table.call_node(expression) else {
                    continue;
                };
                if typed_trees::operator::resolve_named_expression_call(program, call).is_some() {
                    continue;
                }
                covered_expressions.insert(expression);
                if let Some(selection) = selection_for_call(
                    program,
                    machine,
                    state,
                    candidates,
                    callee_states,
                    CallSite::Expression(expression),
                    call.target_symbol,
                    &call.machine_arguments,
                    program.expression_table.expression_handles(call.arguments),
                    expected_calls.iter().find_map(|(candidate, expected)| {
                        (*candidate == expression
                            && (program
                                .typed_program()
                                .callable_body_has_open_parameters(machine)
                                || !static_bindings::names_type_binder(program, *expected)))
                        .then_some(*expected)
                    }),
                    program
                        .typed_program()
                        .callable_body_has_open_parameters(machine),
                ) {
                    upsert_selection(&mut selections, selection);
                }
            }
        }
    }

    // Retained constant initializers are source recipes, not executable sites.
    // Rewriting their detached calls would erase the original application tuple
    // needed by independent constant replay. A private evaluation probe activates
    // its recipe in an executable body, which the scans above still specialize.
    let mut initializer_recipes = Vec::new();
    for declaration in program.const_declarations() {
        if declaration.authored_initializer.is_valid() {
            collect_expression_tree(
                program,
                declaration.authored_initializer,
                &mut initializer_recipes,
            );
        }
    }
    let initializer_recipes: HashSet<ExpressionHandle> = initializer_recipes.into_iter().collect();

    // Calls outside executable states have no caller argument context, but explicit
    // static-machine arguments still determine a complete tuple through the
    // authored machine requirement. Preserve the old all-expression scan for
    // precisely that case.
    for (handle, expression) in program.expression_table.iter_expressions() {
        if covered_expressions.contains(&handle)
            || contract_expressions.contains(&handle)
            || initializer_recipes.contains(&handle)
        {
            continue;
        }
        let ExpressionNode::Call(call) = expression else {
            continue;
        };
        if typed_trees::operator::resolve_named_expression_call(program, call).is_some() {
            continue;
        }
        let Some(callee) = resolve_callee(callee_states, call.target_symbol) else {
            continue;
        };
        let candidate = &candidates[callee.candidate_index];
        let mut proposals = StaticProposals::default();
        // Calls outside executable states have no argument evaluation edge
        // that could carry a runtime subject.
        let explicit_arguments = collect_machine_proposals_for_callee(
            program,
            candidates,
            callee,
            &call.machine_arguments,
            None,
            &mut proposals,
        );
        let lexical_owner = typed_trees::references::StaticArgumentCall::Expression(handle, call)
            .lexical_owner(program);
        // Retained result/capture recipes keep their original lexical owner.
        // A detached occurrence of an open caller's binder is still symbolic;
        // lacking a runtime evaluation edge does not make that owner concrete.
        // Ordinary executable clones are selected above with their real scope.
        let caller_is_generic = program
            .machine_by_symbol(lexical_owner)
            .is_some_and(|owner| {
                program
                    .typed_program()
                    .callable_body_has_open_parameters(owner)
            });
        let mut selection = selection_from_proposals(
            program,
            CallSite::Expression(handle),
            callee,
            candidate,
            caller_is_generic,
            proposals,
        );
        selection.explicit_argument_overflow = explicit_arguments.is_err();
        selection.lexical_owner = lexical_owner;
        upsert_selection(&mut selections, selection);
    }

    selections
}

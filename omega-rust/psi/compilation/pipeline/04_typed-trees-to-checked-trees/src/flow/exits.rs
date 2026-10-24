//! Transition and exit facts. `append_transition_flow_facts` evaluates a guard,
//! then for the target and continuation arms adds the guard outcome (`guards`),
//! evaluates the arm, and records its exit facts. `append_state_exit_facts`
//! records one `FlowExitFact` for each matching contract exit.
use crate::flow::FlowBuildContext;
use crate::flow::append_constraint_ref;
use crate::flow::append_flow_contexts_for_points;
use crate::flow::retained_constraint_refs;
use crate::flow::retained_flow_contexts;
use arena::HandleSpan;
use checked_trees::{
    BorrowCallFact, BorrowFacts, DomainFacts, FlowCallFact, FlowConstraintKind, FlowConstraintRef,
    FlowExitFact, FlowSemanticContextRef, ProofFacts,
};
use facts::{FactPlan, ProgramPoint};
use symbols::SymbolHandle;

mod guards;
pub(super) use guards::{
    append_byte_predicate_context, append_comparison_bounds_context, append_match_pattern_context,
    append_predicate_context,
};

/// Preserve the outgoing source frontier independently from return contracts.
pub(super) fn append_outgoing_state_fact(
    build: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    transition_target: typed_trees::statement::TransitionTargetHandle,
    operand_entry_constraints: HandleSpan<FlowConstraintRef>,
    constraints: HandleSpan<FlowConstraintRef>,
    returned_value: arena::Handle<checked_trees::FlowOperandFact>,
    copied_payloads: HandleSpan<checked_trees::FlowOperandFact>,
) {
    let constraints = retained_constraint_refs(&build.contexts.constraint_refs, constraints);
    build
        .control
        .outgoing
        .append(checked_trees::FlowStateOutgoingFact {
            machine_symbol,
            state_symbol,
            statement_index,
            transition_target,
            returned_value,
            operand_entry_constraints,
            copied_payloads,
            constraints,
        });
}

pub(super) fn append_state_exit_facts(
    program: &dyn ::validation::CheckingInput,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    transition_target: typed_trees::statement::TransitionTargetHandle,
    is_continuation: bool,
    active_contexts: arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: arena::HandleSpan<FlowConstraintRef>,
) -> arena::HandleSpan<FlowExitFact> {
    let mut state_exits = arena::HandleSpan::empty();

    let exit_handles = proof.with_exit_index(|index| {
        index
            .get(&(machine_symbol, state_symbol, transition_target))
            .to_vec()
    });
    for exit_handle in exit_handles {
        let contract_exit = proof.contract_exits.get(exit_handle);
        let outgoing = build
            .control
            .outgoing
            .span_or_empty(
                build
                    .control
                    .outgoing
                    .span_since(build.state_outgoing_start),
            )
            .iter()
            .filter_map(|outgoing| {
                (outgoing.machine_symbol == machine_symbol
                    && outgoing.state_symbol == state_symbol
                    && outgoing.statement_index == contract_exit.statement_index
                    && outgoing.transition_target == transition_target)
                    .then_some(outgoing.returned_value)
            })
            .collect::<Vec<_>>();
        // The capture belongs to one selected exit, not an expression-keyed
        // cache. Ambiguous or absent publication supplies no result evidence.
        let returned_value = match outgoing.as_slice() {
            [capture] => *capture,
            _ => arena::Handle::invalid(),
        };
        if !transition_target.is_valid() && outgoing.is_empty() {
            append_outgoing_state_fact(
                build,
                machine_symbol,
                state_symbol,
                contract_exit.statement_index,
                transition_target,
                active_constraints,
                active_constraints,
                arena::Handle::invalid(),
                HandleSpan::empty(),
            );
        }
        let entry_exit_contexts =
            retained_flow_contexts(&build.contexts.semantic_context_refs, active_contexts);
        let entry_constraints =
            retained_constraint_refs(&build.contexts.constraint_refs, active_constraints);
        let mut ensures_contexts = arena::HandleSpan::empty();
        let mut ensures_constraints = arena::HandleSpan::empty();
        append_flow_contexts_for_points(
            semantic,
            &mut build.contexts.semantic_context_refs,
            &mut ensures_contexts,
            &mut build.contexts.constraint_refs,
            &mut ensures_constraints,
            &[ProgramPoint::Exit {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
                transition_target,
            }],
        );

        let machine = build
            .machine_index(program, machine_symbol)
            .map(|index| &program.machines()[index])
            .expect("exit machine");
        let state = build
            .state_index_in_machine(program, machine_symbol, state_symbol)
            .and_then(|index| program.machine_states(machine).get(index))
            .expect("exit state");
        let (rebased_contexts, parameter_origins) = super::entry_origins::rebase_contexts(
            program,
            semantic,
            build,
            machine,
            state,
            ensures_contexts,
            false,
        );
        ensures_contexts = rebased_contexts;
        ensures_constraints = HandleSpan::empty();
        for context in build
            .contexts
            .semantic_context_refs
            .span_or_empty(ensures_contexts)
            .to_vec()
        {
            append_constraint_ref(
                &mut build.contexts.constraint_refs,
                &mut ensures_constraints,
                FlowConstraintKind::SemanticContext {
                    context: context.context,
                },
            );
        }
        build.control.exits.append_to_span(
            &mut state_exits,
            FlowExitFact {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
                transition_target,
                is_continuation,
                returned_value,
                parameter_origins,
                entry_semantic_contexts: entry_exit_contexts,
                entry_constraints,
                ensures_contexts,
                ensures_constraints,
                ensures: contract_exit.ensures,
            },
        );
    }

    state_exits
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_transition_flow_facts<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    build: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement_fact: arena::Handle<checked_trees::FlowStatementFact>,
    transition: &typed_trees::statement::TableTransition,
    calls: &[BorrowCallFact],
    state_calls: &mut HandleSpan<FlowCallFact>,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) -> super::expression::EvaluatedWritePrefix {
    let mut execution = super::expression::Execution::new(
        program,
        borrow,
        proof,
        domains,
        machine,
        state,
        statement_index,
        statement_fact,
        semantic,
        build,
        state_calls,
        calls,
        active_contexts,
        active_constraints,
    );
    execution.evaluation = checked_trees::FlowPermissionEvaluation::TransitionGuard;
    if let typed_trees::statement::TransitionGuardNode::When(expression) = transition.guard {
        execution.expression(expression, active_contexts, active_constraints);
    }
    // Guards execute on both paths. Their effects survive a missed arm, while
    // calls evaluating one target never run on its sibling continuation.
    let guard_writes = execution.write_prefix();
    let guard_contexts = *active_contexts;
    let guard_constraints = *active_constraints;
    for (is_continuation, target) in [(false, transition.target), (true, transition.continuation)] {
        if !target.is_valid() {
            continue;
        }
        execution.restore_write_prefix(guard_writes);
        let mut branch_contexts = retained_flow_contexts(
            &execution.context.contexts.semantic_context_refs,
            guard_contexts,
        );
        let mut branch_constraints = retained_constraint_refs(
            &execution.context.contexts.constraint_refs,
            guard_constraints,
        );
        guards::append_guard_context(
            program,
            execution.semantic,
            execution.context,
            machine.symbol,
            state.symbol,
            statement_index,
            target,
            transition.guard,
            !is_continuation,
            guard_writes.writes,
            guard_writes.unknown,
            &mut branch_contexts,
            &mut branch_constraints,
        );
        execution.transition_target(
            transition,
            target,
            &mut branch_contexts,
            &mut branch_constraints,
        );
        append_state_exit_facts(
            program,
            proof,
            execution.semantic,
            execution.context,
            machine.symbol,
            state.symbol,
            target,
            is_continuation,
            branch_contexts,
            branch_constraints,
        );
    }
    execution.restore_write_prefix(guard_writes);
    *active_contexts = guard_contexts;
    *active_constraints = guard_constraints;
    guards::append_guard_context(
        program,
        execution.semantic,
        execution.context,
        machine.symbol,
        state.symbol,
        statement_index,
        Default::default(),
        transition.guard,
        false,
        guard_writes.writes,
        guard_writes.unknown,
        active_contexts,
        active_constraints,
    );
    guard_writes
}

//! One state's flow fact. `build_state_flow_fact` collects the declaration and
//! state-entry contexts, rebases them (`entry_origins`), runs each statement
//! (`statements`), drops loans expired at state exit, records the exit facts of a
//! body that ends without a transition (`exits`), and appends the state row.
use crate::flow::FlowBuildContext;
use crate::flow::append_constraint_ref;
use crate::flow::append_contiguous_borrow_root_constraints;
use crate::flow::append_flow_contexts;
use crate::flow::append_flow_contexts_for_points;
use crate::flow::append_state_exit_facts;
use crate::flow::append_state_statement_flow_facts;
use crate::flow::appended_span_since;
use crate::flow::borrow_state_fact;
use crate::flow::filter_expired_borrow_loans;
use crate::flow::project_constraint_refs_to_active_contexts;
use crate::flow::retained_constraint_refs;
use crate::flow::retained_flow_contexts;
use checked_trees::{
    BorrowFacts, DomainFacts, FlowBorrowWeakeningReason, FlowConstraintKind, FlowStateFact,
    ProofFacts,
};
use facts::{FactPlan, ProgramPoint};

pub(super) fn build_state_flow_fact<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    build: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    declaration_groups: [facts::FactContextGroup; 2],
) {
    let Some((borrow_state_handle, borrow_state)) =
        borrow_state_fact(borrow, machine.symbol, state.symbol)
    else {
        return;
    };

    // Store potentials are this pass's evidence; a rebuilt state accumulates
    // them afresh.
    build
        .element_store_potentials
        .retain(|(stored_state, _, _)| *stored_state != state.symbol);

    super::state_values::append_entry_context(program, semantic, build, machine, state);
    let declaration_contexts = declaration_groups
        .into_iter()
        .flat_map(|group| semantic.context_handles_in_group(group));
    // All source-driven checker tests replay the former declaration lookup at
    // this exact point. This catches accidental new Global/Machine producers
    // during flow, including on repeated value-input passes.
    let mut state_contexts = arena::HandleSpan::empty();
    let mut state_constraints = arena::HandleSpan::empty();
    append_flow_contexts(
        declaration_contexts,
        &mut build.contexts.semantic_context_refs,
        &mut state_contexts,
        &mut build.contexts.constraint_refs,
        &mut state_constraints,
    );
    append_flow_contexts_for_points(
        semantic,
        &mut build.contexts.semantic_context_refs,
        &mut state_contexts,
        &mut build.contexts.constraint_refs,
        &mut state_constraints,
        &[ProgramPoint::State {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
        }],
    );
    let (rebased_contexts, _) = {
        super::entry_origins::rebase_contexts(
            program,
            semantic,
            build,
            machine,
            state,
            state_contexts,
            true,
        )
    };
    state_contexts = rebased_contexts;
    state_constraints = project_constraint_refs_to_active_contexts(
        &mut build.contexts.constraint_refs,
        state_constraints,
        state_contexts,
        &build.contexts.semantic_context_refs,
    );
    for context in build
        .contexts
        .semantic_context_refs
        .span_or_empty(state_contexts)
        .to_vec()
    {
        append_constraint_ref(
            &mut build.contexts.constraint_refs,
            &mut state_constraints,
            FlowConstraintKind::SemanticContext {
                context: context.context,
            },
        );
    }
    append_constraint_ref(
        &mut build.contexts.constraint_refs,
        &mut state_constraints,
        FlowConstraintKind::BorrowState {
            state: borrow_state_handle,
        },
    );
    append_contiguous_borrow_root_constraints(
        &mut build.contexts.constraint_refs,
        &mut state_constraints,
        borrow_state.writable_roots,
    );
    // Requires constrain the incoming storage, not every later version of it.
    // Materialize their bounds before statements so selected assignments can
    // capture them and ordinary write invalidation retires the old observation.
    // Machine requires apply only at the entry state, never afresh at each
    // state of the machine.
    let is_entry = program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol);
    let state_origin = facts::FactOrigin::StateContract {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
    };
    let machine_origin = facts::FactOrigin::MachineContract {
        machine_symbol: machine.symbol,
    };
    for (contract, origin) in program
        .state_contracts(state)
        .iter()
        .map(|contract| (contract, state_origin))
        .chain(
            program
                .machine_contracts(machine)
                .iter()
                .filter(|_| is_entry)
                .map(|contract| (contract, machine_origin)),
        )
        .filter(|(contract, _)| {
            contract.kind == typed_trees::signature::SignatureContractKind::Requires
        })
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(expression) = fact.as_expression() else {
                continue;
            };
            // A `requires pred(place)` clause is the callee's own assumption:
            // the caller proves the probe, the state enters carrying the
            // per-byte class at the operand's place — the same whole-carrier
            // fact a validating guard mints.
            super::exits::append_byte_predicate_context(
                program,
                semantic,
                build,
                state.symbol,
                0,
                *expression,
                ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                },
                origin,
                &mut state_contexts,
                &mut state_constraints,
            );
            // In particular, do not peel a Boolean equality whose selected
            // operator does not mean builtin equality.
            if !validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                *expression,
            ) {
                continue;
            }
            super::exits::append_comparison_bounds_context(
                program,
                semantic,
                build,
                machine.symbol,
                state.symbol,
                0,
                *expression,
                ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                },
                origin,
                &mut state_contexts,
                &mut state_constraints,
            );
        }
    }
    let mut active_contexts =
        retained_flow_contexts(&build.contexts.semantic_context_refs, state_contexts);
    let mut active_constraints =
        retained_constraint_refs(&build.contexts.constraint_refs, state_constraints);
    let state_invalidations_start = build.invalidations.events.len();
    let state_borrow_activations_start = build.borrow_lifetimes.activations.len();
    let state_borrow_weakenings_start = build.borrow_lifetimes.weakenings.len();
    let state_boundary_edges_start = build.boundaries.edges.len();
    let state_statements_start = build.control.statements.len();
    let state_case_accesses_start = build.control.case_accesses.len();
    let state_exits_start = build.control.exits.len();
    let state_outgoing_start = build.control.outgoing.len();
    let execution_order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let state_calls = {
        append_state_statement_flow_facts(
            program,
            borrow,
            proof,
            semantic,
            domains,
            build,
            machine,
            state,
            &mut active_contexts,
            &mut active_constraints,
            borrow_state,
            &execution_order,
        )
    };
    active_constraints = filter_expired_borrow_loans(
        &mut build.borrow_lifetimes.weakenings,
        &mut build.contexts.constraint_refs,
        active_constraints,
        borrow,
        program
            .statement_table
            .statements(state.statement_nodes)
            .len(),
        &execution_order,
        FlowBorrowWeakeningReason::StateExit,
    );
    append_state_exit_facts(
        program,
        proof,
        semantic,
        build,
        machine.symbol,
        state.symbol,
        Default::default(),
        active_contexts,
        active_constraints,
    );

    build.control.states.append(FlowStateFact {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        writable_roots: borrow_state.writable_roots,
        mutable_parameter_count: borrow_state.mutable_parameter_count,
        entry_semantic_contexts: state_contexts,
        entry_constraints: state_constraints,
        invalidations: appended_span_since(&build.invalidations.events, state_invalidations_start),
        borrow_activations: appended_span_since(
            &build.borrow_lifetimes.activations,
            state_borrow_activations_start,
        ),
        borrow_weakenings: appended_span_since(
            &build.borrow_lifetimes.weakenings,
            state_borrow_weakenings_start,
        ),
        boundary_edges: appended_span_since(&build.boundaries.edges, state_boundary_edges_start),
        statements: appended_span_since(&build.control.statements, state_statements_start),
        case_accesses: appended_span_since(&build.control.case_accesses, state_case_accesses_start),
        calls: state_calls,
        exits: appended_span_since(&build.control.exits, state_exits_start),
        outgoing: appended_span_since(&build.control.outgoing, state_outgoing_start),
        service_reach: Default::default(),
        suspension: Default::default(),
        blocking: Default::default(),
    });
}

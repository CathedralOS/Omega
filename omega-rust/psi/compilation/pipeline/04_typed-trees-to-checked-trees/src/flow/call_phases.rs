//! The phases `calls::build_call_flow_fact` runs, in its order: entry contexts with
//! the call's borrow constraints, `requires` contexts, invalidation over the call's
//! written places (`invalidation`), `ensures` exit contexts, and field facts
//! re-seeded on readable `&mut` referents (`referents`).
use crate::flow::FlowBuildContext;
use crate::flow::append_constraint_ref;
use crate::flow::append_contiguous_borrow_access_constraints;
use crate::flow::append_flow_contexts_for_points;
use crate::flow::retained_constraint_refs;
use crate::flow::retained_flow_contexts;
use arena::HandleSpan;
use checked_trees::{
    BorrowCallFact, BorrowFacts, FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef,
};
use facts::{FactPlan, ProgramPoint};

mod copied_guarantees;
mod invalidation;
mod referents;

pub(super) use copied_guarantees::ArgumentSources;
pub(super) use invalidation::{apply_call_invalidations, call_storage_writes};
pub(super) use referents::append_call_referent_field_domain_facts;

pub(super) struct CallFlowContexts {
    pub(super) contexts: HandleSpan<FlowSemanticContextRef>,
    pub(super) constraints: HandleSpan<FlowConstraintRef>,
}

pub(super) fn build_call_entry_contexts(
    borrow: &BorrowFacts,
    build: &mut FlowBuildContext,
    active_contexts: HandleSpan<FlowSemanticContextRef>,
    active_constraints: HandleSpan<FlowConstraintRef>,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    borrow_call: &BorrowCallFact,
) -> CallFlowContexts {
    let contexts = retained_flow_contexts(&build.contexts.semantic_context_refs, active_contexts);
    let mut constraints =
        retained_constraint_refs(&build.contexts.constraint_refs, active_constraints);
    // Coordinates repeat across states. The state owns the call identity;
    // a global first-match can attach another invocation's authority.
    let owned_call = crate::flow::fact_rows::with_borrow_state_index(borrow, |index| {
        index.bucket(machine_symbol, state_symbol).to_vec()
    })
    .into_iter()
    .find_map(|state_handle| {
        let state = borrow.states.get(state_handle);
        borrow
            .calls
            .span_or_empty(state.calls)
            .iter()
            .position(|call| call == borrow_call)
            .map(|offset| state.calls.handle_at(offset as u32))
    });
    if let Some(borrow_call_handle) = owned_call {
        append_constraint_ref(
            &mut build.contexts.constraint_refs,
            &mut constraints,
            FlowConstraintKind::BorrowCall {
                call: borrow_call_handle,
            },
        );
    }
    append_contiguous_borrow_access_constraints(
        &mut build.contexts.constraint_refs,
        &mut constraints,
        borrow_call.accesses,
    );

    CallFlowContexts {
        contexts,
        constraints,
    }
}

pub(super) fn build_call_requires_contexts(
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) -> CallFlowContexts {
    build_call_contract_contexts(
        semantic,
        build,
        ProgramPoint::CallRequires {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        },
    )
}

pub(super) fn build_call_exit_contexts(
    program: &dyn ::validation::CheckingInput,
    proof: &checked_trees::ProofFacts,
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
    post_contexts: HandleSpan<FlowSemanticContextRef>,
    post_constraints: HandleSpan<FlowConstraintRef>,
    argument_sources: Option<&ArgumentSources<'_, '_>>,
) -> CallFlowContexts {
    // Only newly substituted callee guarantees can confuse a captured value
    // with its former storage. Keep the live caller contexts after the call's
    // own invalidation; later operands may already have restored their domains.
    let mut ensured = build_call_contract_contexts(
        semantic,
        build,
        ProgramPoint::CallEnsures {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        },
    );
    let retained_contexts = copied_guarantees::retain_guarantees(
        program,
        proof,
        semantic,
        build,
        super::proof_contract_call(
            proof,
            machine.symbol,
            state.symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
        ),
        argument_sources,
        ensured.contexts,
    );
    // The contract builder appended each context and its constraint together.
    // Retention only selects existing rows; an unchanged span keeps the same
    // aligned pair without repeating a membership query for every constraint.
    if retained_contexts != ensured.contexts {
        ensured.contexts = retained_contexts;
        ensured.constraints = super::project_constraint_refs_to_active_contexts(
            &mut build.contexts.constraint_refs,
            ensured.constraints,
            ensured.contexts,
            &build.contexts.semantic_context_refs,
        );
    }
    let mut contexts = retained_flow_contexts(&build.contexts.semantic_context_refs, post_contexts);
    let mut constraints =
        retained_constraint_refs(&build.contexts.constraint_refs, post_constraints);
    let ensured_contexts = build
        .contexts
        .semantic_context_refs
        .span_or_empty(ensured.contexts)
        .to_vec();
    for reference in ensured_contexts {
        super::reference_spans::append_flow_reference(
            &mut build.contexts.semantic_context_refs,
            &mut contexts,
            reference,
        );
    }
    let ensured_constraints = build
        .contexts
        .constraint_refs
        .span_or_empty(ensured.constraints)
        .to_vec();
    for reference in ensured_constraints {
        super::reference_spans::append_flow_reference(
            &mut build.contexts.constraint_refs,
            &mut constraints,
            reference,
        );
    }

    CallFlowContexts {
        contexts,
        constraints,
    }
}

fn build_call_contract_contexts(
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    point: ProgramPoint,
) -> CallFlowContexts {
    let mut contexts = HandleSpan::empty();
    let mut constraints = HandleSpan::empty();
    append_call_contract_contexts(semantic, build, &mut contexts, &mut constraints, point);
    CallFlowContexts {
        contexts,
        constraints,
    }
}

fn append_call_contract_contexts(
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    contexts: &mut HandleSpan<FlowSemanticContextRef>,
    constraints: &mut HandleSpan<FlowConstraintRef>,
    point: ProgramPoint,
) {
    append_flow_contexts_for_points(
        semantic,
        &mut build.contexts.semantic_context_refs,
        contexts,
        &mut build.contexts.constraint_refs,
        constraints,
        &[point],
    );
}

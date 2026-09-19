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

mod invalidation;
mod referents;

pub(super) use invalidation::{apply_call_invalidations, call_storage_writes};
pub(super) use referents::append_call_referent_field_domain_facts;

pub(super) struct CallFlowContexts {
    pub(super) contexts: HandleSpan<FlowSemanticContextRef>,
    pub(super) constraints: HandleSpan<FlowConstraintRef>,
}

pub(super) fn build_call_entry_contexts(
    borrow: &BorrowFacts,
    ctx: &mut FlowBuildContext,
    active_contexts: HandleSpan<FlowSemanticContextRef>,
    active_constraints: HandleSpan<FlowConstraintRef>,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    borrow_call: &BorrowCallFact,
) -> CallFlowContexts {
    let contexts = retained_flow_contexts(&ctx.contexts.semantic_context_refs, active_contexts);
    let mut constraints =
        retained_constraint_refs(&ctx.contexts.constraint_refs, active_constraints);
    // Coordinates repeat across states. The state owns the call identity;
    // a global first-match can attach another invocation's authority.
    let owned_call = borrow.states.iter().find_map(|(_, state)| {
        if state.machine_symbol != machine_symbol || state.state_symbol != state_symbol {
            return None;
        }
        borrow
            .calls
            .span_or_empty(state.calls)
            .iter()
            .position(|call| call == borrow_call)
            .map(|offset| {
                arena::Handle::from_parts(
                    state.calls.start().arena_index() + offset as u32,
                    state.calls.start().generation(),
                )
            })
    });
    if let Some(borrow_call_handle) = owned_call {
        append_constraint_ref(
            &mut ctx.contexts.constraint_refs,
            &mut constraints,
            FlowConstraintKind::BorrowCall {
                call: borrow_call_handle,
            },
        );
    }
    append_contiguous_borrow_access_constraints(
        &mut ctx.contexts.constraint_refs,
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
    ctx: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) -> CallFlowContexts {
    build_call_contract_contexts(
        semantic,
        ctx,
        ProgramPoint::CallRequires {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        },
    )
}

pub(super) fn build_call_exit_contexts(
    semantic: &FactPlan,
    ctx: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
    post_contexts: HandleSpan<FlowSemanticContextRef>,
    post_constraints: HandleSpan<FlowConstraintRef>,
) -> CallFlowContexts {
    let mut contexts = retained_flow_contexts(&ctx.contexts.semantic_context_refs, post_contexts);
    let mut constraints = retained_constraint_refs(&ctx.contexts.constraint_refs, post_constraints);
    append_call_contract_contexts(
        semantic,
        ctx,
        &mut contexts,
        &mut constraints,
        ProgramPoint::CallEnsures {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        },
    );

    CallFlowContexts {
        contexts,
        constraints,
    }
}

fn build_call_contract_contexts(
    semantic: &FactPlan,
    ctx: &mut FlowBuildContext,
    point: ProgramPoint,
) -> CallFlowContexts {
    let mut contexts = HandleSpan::empty();
    let mut constraints = HandleSpan::empty();
    append_call_contract_contexts(semantic, ctx, &mut contexts, &mut constraints, point);
    CallFlowContexts {
        contexts,
        constraints,
    }
}

fn append_call_contract_contexts(
    semantic: &FactPlan,
    ctx: &mut FlowBuildContext,
    contexts: &mut HandleSpan<FlowSemanticContextRef>,
    constraints: &mut HandleSpan<FlowConstraintRef>,
    point: ProgramPoint,
) {
    append_flow_contexts_for_points(
        semantic,
        &mut ctx.contexts.semantic_context_refs,
        contexts,
        &mut ctx.contexts.constraint_refs,
        constraints,
        &[point],
    );
}

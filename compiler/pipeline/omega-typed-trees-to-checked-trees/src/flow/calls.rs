use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_call_flow_fact(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
    ctx: &mut FlowBuildContext,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    state_effects: Option<&omega_effects::StateEffects>,
    active_contexts: &mut omega_core::arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut omega_core::arena::HandleSpan<FlowConstraintRef>,
    borrow_call: &BorrowCallFact,
) -> FlowCallFact {
    let effect_call = effects_call(effects, state_effects, borrow_call);
    let contract_call = proof_contract_call(
        proof,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    );
    let entry_contexts = clone_flow_contexts(&mut ctx.semantic_context_refs, *active_contexts);
    let mut entry_constraints =
        clone_constraint_refs(&mut ctx.constraint_refs, *active_constraints);
    if let Some((borrow_call_handle, _)) = borrow.calls.iter().find(|(_, call)| {
        call.statement_index == borrow_call.statement_index
            && call.call_ordinal == borrow_call.call_ordinal
            && call.target_symbol == borrow_call.target_symbol
            && call.receiver_symbol == borrow_call.receiver_symbol
    }) {
        append_constraint_ref(
            &mut ctx.constraint_refs,
            &mut entry_constraints,
            FlowConstraintKind::BorrowCall {
                call: borrow_call_handle,
            },
        );
    }
    append_contiguous_borrow_access_constraints(
        &mut ctx.constraint_refs,
        &mut entry_constraints,
        borrow_call.accesses,
    );
    let mut requires_contexts = omega_core::arena::HandleSpan::empty();
    let mut requires_constraints = omega_core::arena::HandleSpan::empty();
    append_flow_contexts_for_points(
        semantic,
        &mut ctx.semantic_context_refs,
        &mut requires_contexts,
        &[ProgramPoint::CallRequires {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        }],
    );
    append_semantic_constraints_for_points(
        semantic,
        &mut ctx.constraint_refs,
        &mut requires_constraints,
        &[ProgramPoint::CallRequires {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        }],
    );
    let mutated_places = call_mutated_places(
        program,
        machine.symbol,
        state.symbol,
        borrow,
        borrow_call,
        &mut ctx.state_mutation_summary_cache,
    );
    let call_invalidations_start = ctx.invalidations.len();
    let post_call_contexts = if call_may_mutate_contract_state(program, borrow, borrow_call) {
        if mutated_places.is_empty() {
            omega_core::arena::HandleSpan::empty()
        } else {
            filter_contexts_after_place_mutations(
                program,
                semantic,
                domains,
                &mut ctx.semantic_context_refs,
                &mut ctx.invalidation_segments,
                &mut ctx.invalidations,
                *active_contexts,
                &mutated_places,
                FlowInvalidationSource::Call {
                    statement_index: borrow_call.statement_index,
                    call_ordinal: borrow_call.call_ordinal,
                    target_symbol: borrow_call.target_symbol,
                },
            )
        }
    } else {
        clone_flow_contexts(&mut ctx.semantic_context_refs, *active_contexts)
    };
    let post_call_constraints = project_constraint_refs_to_active_contexts(
        &mut ctx.constraint_refs,
        *active_constraints,
        post_call_contexts,
        &ctx.semantic_context_refs,
    );
    let call_invalidations = appended_span_since(&ctx.invalidations, call_invalidations_start);
    let mut exit_contexts = clone_flow_contexts(&mut ctx.semantic_context_refs, post_call_contexts);
    let mut exit_constraints =
        clone_constraint_refs(&mut ctx.constraint_refs, post_call_constraints);
    append_flow_contexts_for_points(
        semantic,
        &mut ctx.semantic_context_refs,
        &mut exit_contexts,
        &[ProgramPoint::CallEnsures {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        }],
    );
    append_semantic_constraints_for_points(
        semantic,
        &mut ctx.constraint_refs,
        &mut exit_constraints,
        &[ProgramPoint::CallEnsures {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        }],
    );
    append_call_ownership_events(program, ctx, state, borrow_call);
    let boundary_edges = append_call_boundary_edges(program, ctx, borrow_call);
    *active_contexts = clone_flow_contexts(&mut ctx.semantic_context_refs, exit_contexts);
    *active_constraints = clone_constraint_refs(&mut ctx.constraint_refs, exit_constraints);

    FlowCallFact {
        statement_index: borrow_call.statement_index,
        call_ordinal: borrow_call.call_ordinal,
        receiver_symbol: borrow_call.receiver_symbol,
        target_symbol: borrow_call.target_symbol,
        has_receiver: borrow_call.has_receiver,
        accesses: borrow_call.accesses,
        entry_semantic_contexts: entry_contexts,
        entry_constraints,
        requires_contexts,
        requires_constraints,
        exit_semantic_contexts: exit_contexts,
        exit_constraints,
        invalidations: call_invalidations,
        boundary_edges,
        requires: contract_call
            .map(|call| call.requires)
            .unwrap_or_else(HandleSpan::empty),
        ensures: contract_call
            .map(|call| call.ensures)
            .unwrap_or_else(HandleSpan::empty),
        direct_effects: effect_call
            .map(|call| call.direct)
            .unwrap_or_else(omega_effects::EffectSet::empty),
        transitive_effects: effect_call
            .map(|call| call.transitive)
            .unwrap_or_else(omega_effects::EffectSet::empty),
    }
}

fn append_call_boundary_edges(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    borrow_call: &BorrowCallFact,
) -> HandleSpan<FlowBoundaryEdgeFact> {
    let Some((target_machine, target_state)) =
        target_machine_and_state(program, borrow_call.target_symbol)
    else {
        return HandleSpan::empty();
    };

    let mut span = HandleSpan::empty();
    let mut visited_traits = Vec::new();
    for conformance in program.machine_trait_conformances(target_machine) {
        append_boundary_edges_for_trait(
            program,
            ctx,
            borrow_call,
            target_state,
            conformance.symbol,
            &mut visited_traits,
            &mut span,
        );
    }

    span
}

fn append_boundary_edges_for_trait(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    borrow_call: &BorrowCallFact,
    target_state: &omega_typed_trees::state::State,
    trait_symbol: SymbolHandle,
    visited_traits: &mut Vec<SymbolHandle>,
    span: &mut HandleSpan<FlowBoundaryEdgeFact>,
) {
    if !trait_symbol.is_valid() || visited_traits.iter().any(|symbol| *symbol == trait_symbol) {
        return;
    }

    let Some(trait_definition) = program
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.symbol == trait_symbol)
    else {
        return;
    };

    visited_traits.push(trait_symbol);

    if trait_definition.is_boundary {
        for signature in program
            .trait_machine_signatures(trait_definition)
            .iter()
            .filter(|signature| signature.name == target_state.name)
        {
            ctx.boundary_edges.append_to_span(
                span,
                FlowBoundaryEdgeFact {
                    statement_index: borrow_call.statement_index,
                    call_ordinal: borrow_call.call_ordinal,
                    receiver_symbol: borrow_call.receiver_symbol,
                    target_symbol: borrow_call.target_symbol,
                    boundary_trait_symbol: trait_definition.symbol,
                    boundary_signature_symbol: signature.symbol,
                },
            );
        }
    }

    for requirement in program.trait_requirements(trait_definition) {
        append_boundary_edges_for_trait(
            program,
            ctx,
            borrow_call,
            target_state,
            requirement.symbol,
            visited_traits,
            span,
        );
    }

    visited_traits.pop();
}

fn target_machine_and_state(
    program: &omega_typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<(
    &omega_typed_trees::machine::Machine,
    &omega_typed_trees::state::State,
)> {
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target_state_symbol)
            .map(|state| (machine, state))
    })
}

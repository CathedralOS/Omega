use super::*;

pub(crate) fn build_flow_facts(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
) -> FlowFacts {
    let mut ctx = FlowBuildContext::new(borrow, proof, semantic);

    for machine in program.machines() {
        let machine_effects = effects_machine(effects, machine.symbol);

        for state in program.machine_states(machine) {
            build_state_flow_fact(
                program,
                borrow,
                proof,
                semantic,
                domains,
                effects,
                &mut ctx,
                machine,
                state,
                machine_effects,
            );
        }
    }

    ctx.finish()
}

fn build_state_flow_fact(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
    ctx: &mut FlowBuildContext,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    machine_effects: Option<&omega_effects::MachineEffects>,
) {
    let Some((borrow_state_handle, borrow_state)) =
        borrow_state_fact(borrow, machine.symbol, state.symbol)
    else {
        return;
    };

    let state_effects = effects_state(effects, machine_effects, state.symbol);
    let mut state_contexts = omega_core::arena::HandleSpan::empty();
    let mut state_constraints = omega_core::arena::HandleSpan::empty();
    append_flow_contexts_for_points(
        semantic,
        &mut ctx.semantic_context_refs,
        &mut state_contexts,
        &[
            ProgramPoint::Global,
            ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            },
            ProgramPoint::State {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            },
        ],
    );
    append_semantic_constraints_for_points(
        semantic,
        &mut ctx.constraint_refs,
        &mut state_constraints,
        &[
            ProgramPoint::Global,
            ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            },
            ProgramPoint::State {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            },
        ],
    );
    append_constraint_ref(
        &mut ctx.constraint_refs,
        &mut state_constraints,
        FlowConstraintKind::BorrowState {
            state: borrow_state_handle,
        },
    );
    append_contiguous_borrow_root_constraints(
        &mut ctx.constraint_refs,
        &mut state_constraints,
        borrow_state.writable_roots,
    );
    let mut active_contexts = clone_flow_contexts(&mut ctx.semantic_context_refs, state_contexts);
    let mut active_constraints = clone_constraint_refs(&mut ctx.constraint_refs, state_constraints);
    let state_invalidations_start = ctx.invalidations.len();
    let state_borrow_activations_start = ctx.borrow_activations.len();
    let state_borrow_weakenings_start = ctx.borrow_weakenings.len();
    let state_statements_start = ctx.statements.len();
    let state_calls = append_state_call_facts(
        program,
        borrow,
        proof,
        semantic,
        domains,
        effects,
        ctx,
        machine,
        state,
        state_effects,
        &mut active_contexts,
        &mut active_constraints,
        borrow_state,
    );
    active_constraints = filter_expired_borrow_loans(
        &mut ctx.borrow_weakenings,
        &mut ctx.constraint_refs,
        active_constraints,
        borrow,
        program
            .statement_table
            .statements(state.statement_nodes)
            .len(),
        FlowBorrowWeakeningReason::StateExit,
    );
    let state_exits = append_state_exit_facts(
        proof,
        semantic,
        ctx,
        machine.symbol,
        state.symbol,
        active_contexts,
        active_constraints,
    );

    ctx.states.append(FlowStateFact {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        writable_roots: borrow_state.writable_roots,
        mutable_parameter_count: borrow_state.mutable_parameter_count,
        entry_semantic_contexts: state_contexts,
        entry_constraints: state_constraints,
        invalidations: appended_span_since(&ctx.invalidations, state_invalidations_start),
        borrow_activations: appended_span_since(
            &ctx.borrow_activations,
            state_borrow_activations_start,
        ),
        borrow_weakenings: appended_span_since(
            &ctx.borrow_weakenings,
            state_borrow_weakenings_start,
        ),
        statements: appended_span_since(&ctx.statements, state_statements_start),
        calls: state_calls,
        exits: state_exits,
        direct_effects: state_effects
            .map(|state_effects| state_effects.direct)
            .unwrap_or_else(omega_effects::EffectSet::empty),
        transitive_effects: state_effects
            .map(|state_effects| state_effects.transitive)
            .unwrap_or_else(omega_effects::EffectSet::empty),
    });
}

#[allow(clippy::too_many_arguments)]
fn append_state_call_facts(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
    ctx: &mut FlowBuildContext,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    state_effects: Option<&omega_effects::StateEffects>,
    active_contexts: &mut omega_core::arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut omega_core::arena::HandleSpan<FlowConstraintRef>,
    borrow_state: &StateBorrowFact,
) -> omega_core::arena::HandleSpan<FlowCallFact> {
    let mut state_calls = omega_core::arena::HandleSpan::empty();
    let borrow_calls = borrow.calls.span_or_empty(borrow_state.calls);
    let borrow_loans = borrow.loans.span_or_empty(borrow_state.loans);
    let mut call_index = 0usize;
    let mut loan_index = 0usize;

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        *active_constraints = filter_expired_borrow_loans(
            &mut ctx.borrow_weakenings,
            &mut ctx.constraint_refs,
            *active_constraints,
            borrow,
            statement_index,
            FlowBorrowWeakeningReason::LastUseExpired,
        );
        ctx.statements.append(FlowStatementFact {
            statement_index,
            entry_semantic_contexts: *active_contexts,
            entry_constraints: *active_constraints,
        });

        while let Some(borrow_call) = borrow_calls.get(call_index) {
            if borrow_call.statement_index != statement_index {
                break;
            }
            call_index += 1;

            let call_flow = build_call_flow_fact(
                program,
                borrow,
                proof,
                semantic,
                domains,
                effects,
                ctx,
                machine,
                state,
                state_effects,
                active_contexts,
                active_constraints,
                borrow_call,
            );
            ctx.calls.append_to_span(&mut state_calls, call_flow);
        }

        while let Some(loan) = borrow_loans.get(loan_index) {
            if loan.statement_index != statement_index {
                break;
            }
            loan_index += 1;
            let loan_handle = Handle::from_parts(
                borrow_state
                    .loans
                    .start()
                    .arena_index()
                    .saturating_add((loan_index - 1) as u32),
                borrow_state.loans.start().generation(),
            );

            ctx.borrow_activations.append(FlowBorrowActivationFact {
                source: FlowInvalidationSource::Statement { statement_index },
                loan: loan_handle,
            });

            append_constraint_ref(
                &mut ctx.constraint_refs,
                active_constraints,
                FlowConstraintKind::BorrowLoan { loan: loan_handle },
            );
        }

        if let Some(place) = statement_mutated_place(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            statement,
        ) {
            *active_contexts = filter_contexts_after_place_mutations(
                program,
                semantic,
                domains,
                &mut ctx.semantic_context_refs,
                &mut ctx.invalidation_segments,
                &mut ctx.invalidations,
                *active_contexts,
                &[place],
                FlowInvalidationSource::Statement { statement_index },
            );
            *active_constraints = project_constraint_refs_to_active_contexts(
                &mut ctx.constraint_refs,
                *active_constraints,
                *active_contexts,
                &ctx.semantic_context_refs,
            );
        }

        *active_constraints = filter_reassigned_borrow_loans(
            &mut ctx.borrow_weakenings,
            &mut ctx.constraint_refs,
            *active_constraints,
            borrow,
            program,
            state.symbol,
            statement_index,
            statement,
        );

        propagate_statement_transfers(
            program,
            semantic,
            ctx,
            machine.symbol,
            state.symbol,
            statement_index,
            statement,
            active_contexts,
            active_constraints,
        );
    }

    state_calls
}

fn append_state_exit_facts(
    proof: &ProofFacts,
    semantic: &FactPlan,
    ctx: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    active_contexts: omega_core::arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: omega_core::arena::HandleSpan<FlowConstraintRef>,
) -> omega_core::arena::HandleSpan<FlowExitFact> {
    let mut state_exits = omega_core::arena::HandleSpan::empty();

    for contract_exit in proof.contract_exits.iter().filter_map(|(_, exit)| {
        (exit.machine_symbol == machine_symbol && exit.state_symbol == state_symbol).then_some(exit)
    }) {
        let entry_exit_contexts =
            clone_flow_contexts(&mut ctx.semantic_context_refs, active_contexts);
        let entry_constraints = clone_constraint_refs(&mut ctx.constraint_refs, active_constraints);
        let mut ensures_contexts = omega_core::arena::HandleSpan::empty();
        let mut ensures_constraints = omega_core::arena::HandleSpan::empty();
        append_flow_contexts_for_points(
            semantic,
            &mut ctx.semantic_context_refs,
            &mut ensures_contexts,
            &[ProgramPoint::Exit {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
            }],
        );
        append_semantic_constraints_for_points(
            semantic,
            &mut ctx.constraint_refs,
            &mut ensures_constraints,
            &[ProgramPoint::Exit {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
            }],
        );

        ctx.exits.append_to_span(
            &mut state_exits,
            FlowExitFact {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
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

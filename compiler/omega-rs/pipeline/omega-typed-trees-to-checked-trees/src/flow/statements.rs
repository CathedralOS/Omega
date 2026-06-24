use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_state_statement_flow_facts(
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
            &mut ctx.borrow_lifetimes.weakenings,
            &mut ctx.contexts.constraint_refs,
            *active_constraints,
            borrow,
            statement_index,
            FlowBorrowWeakeningReason::LastUseExpired,
        );
        ctx.control.statements.append(FlowStatementFact {
            statement_index,
            entry_semantic_contexts: *active_contexts,
            entry_constraints: *active_constraints,
        });
        // A multi-arm transition desugars to a SEQUENCE of guarded transition
        // statements (an if/elseif chain): statement N is taken when its guard
        // holds (and the state exits), else control falls through to statement
        // N+1. A guarded transition's TARGET call exits the state, so threading
        // its (empty) exit context to the next statement is wrong -- the next
        // statement is the FALLTHROUGH (this guard was false), which never ran the
        // branch, so it must keep the PRE-transition context. Without this, an
        // else-arm call (`false -> consume(text)`) saw 0 entry contexts and a
        // forwarded domain fact never reached it. Snapshot the entry context so we
        // can restore it for the fallthrough after this statement is flowed.
        let fallthrough_contexts = *active_contexts;
        let fallthrough_constraints = *active_constraints;
        append_statement_ownership_events(program, ctx, state.symbol, statement_index, statement);

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
            ctx.control
                .calls
                .append_to_span(&mut state_calls, call_flow);
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

            ctx.borrow_lifetimes
                .activations
                .append(FlowBorrowActivationFact {
                    source: FlowInvalidationSource::Statement { statement_index },
                    loan: loan_handle,
                });

            append_constraint_ref(
                &mut ctx.contexts.constraint_refs,
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
                &mut ctx.contexts.semantic_context_refs,
                &mut ctx.invalidations.segments,
                &mut ctx.invalidations.events,
                *active_contexts,
                &[place],
                FlowInvalidationSource::Statement { statement_index },
            );
            *active_constraints = project_constraint_refs_to_active_contexts(
                &mut ctx.contexts.constraint_refs,
                *active_constraints,
                *active_contexts,
                &ctx.contexts.semantic_context_refs,
            );
        }

        *active_constraints = filter_reassigned_borrow_loans(
            &mut ctx.borrow_lifetimes.weakenings,
            &mut ctx.contexts.constraint_refs,
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

        // Restore the pre-transition context for the fallthrough path (see the
        // snapshot above). A guarded transition either takes its branch and exits
        // -- so its exit context belongs to the target state's flow, not to the
        // sibling fallthrough -- or its guard is false and control continues to the
        // next statement with the context unchanged by the (untaken) branch.
        if matches!(
            statement,
            omega_typed_trees::statement::StatementNode::Transition(_)
        ) {
            *active_contexts = fallthrough_contexts;
            *active_constraints = fallthrough_constraints;
        }
    }

    state_calls
}

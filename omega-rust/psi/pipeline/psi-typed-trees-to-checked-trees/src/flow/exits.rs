use super::*;

mod guards;

pub(super) fn append_state_exit_facts(
    program: &psi_typed_trees::TypedTrees,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    transition_target: psi_typed_trees::statement::TransitionTargetHandle,
    active_contexts: psi_arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: psi_arena::HandleSpan<FlowConstraintRef>,
) -> psi_arena::HandleSpan<FlowExitFact> {
    let mut state_exits = psi_arena::HandleSpan::empty();

    for contract_exit in proof.contract_exits.iter().filter_map(|(_, exit)| {
        (exit.machine_symbol == machine_symbol
            && exit.state_symbol == state_symbol
            && exit.transition_target == transition_target)
            .then_some(exit)
    }) {
        let entry_exit_contexts =
            clone_flow_contexts(&mut ctx.contexts.semantic_context_refs, active_contexts);
        let entry_constraints =
            clone_constraint_refs(&mut ctx.contexts.constraint_refs, active_constraints);
        let mut ensures_contexts = psi_arena::HandleSpan::empty();
        let mut ensures_constraints = psi_arena::HandleSpan::empty();
        append_flow_contexts_for_points(
            semantic,
            &mut ctx.contexts.semantic_context_refs,
            &mut ensures_contexts,
            &[ProgramPoint::Exit {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
                transition_target,
            }],
        );
        append_semantic_constraints_for_points(
            semantic,
            &mut ctx.contexts.constraint_refs,
            &mut ensures_constraints,
            &[ProgramPoint::Exit {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
                transition_target,
            }],
        );

        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
            .expect("exit machine");
        let state = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
            .expect("exit state");
        let (rebased_contexts, parameter_origins) = super::entry_origins::rebase_contexts(
            program,
            semantic,
            ctx,
            machine,
            state,
            ensures_contexts,
            false,
        );
        ensures_contexts = rebased_contexts;
        ensures_constraints = HandleSpan::empty();
        for context in ctx
            .contexts
            .semantic_context_refs
            .span_or_empty(ensures_contexts)
            .to_vec()
        {
            append_constraint_ref(
                &mut ctx.contexts.constraint_refs,
                &mut ensures_constraints,
                FlowConstraintKind::SemanticContext {
                    context: context.context,
                },
            );
        }
        ctx.control.exits.append_to_span(
            &mut state_exits,
            FlowExitFact {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
                transition_target,
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
pub(super) fn append_transition_flow_facts(
    program: &psi_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    ctx: &mut FlowBuildContext,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statement_index: usize,
    transition: &psi_typed_trees::statement::TableTransition,
    calls: &[BorrowCallFact],
    state_calls: &mut HandleSpan<FlowCallFact>,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let mut regions = [Vec::new(), Vec::new(), Vec::new()];
    for call in calls {
        let target = crate::semantic_calls::transition_call_target(
            program,
            machine,
            state,
            statement_index,
            call.call_ordinal,
        );
        let region = match target {
            Some(target) if target == transition.target => 1,
            Some(target) if target.is_valid() && target == transition.continuation => 2,
            Some(_) => 0,
            None => {
                // A malformed call coordinate supplies no branch-local proof.
                *active_contexts = HandleSpan::empty();
                *active_constraints = HandleSpan::empty();
                0
            }
        };
        regions[region].push(call);
    }
    for call in &regions[0] {
        let flow = build_call_flow_fact(
            program,
            borrow,
            proof,
            semantic,
            domains,
            ctx,
            machine,
            state,
            active_contexts,
            active_constraints,
            call,
        );
        ctx.control.calls.append_to_span(state_calls, flow);
    }
    // Guards execute on both paths. Their effects survive a missed arm, while
    // calls evaluating one target never run on its sibling continuation.
    let guard_contexts = *active_contexts;
    let guard_constraints = *active_constraints;
    for (region, target) in [(1, transition.target), (2, transition.continuation)] {
        if !target.is_valid() {
            continue;
        }
        let mut branch_contexts =
            clone_flow_contexts(&mut ctx.contexts.semantic_context_refs, guard_contexts);
        let mut branch_constraints =
            clone_constraint_refs(&mut ctx.contexts.constraint_refs, guard_constraints);
        guards::append_guard_context(
            program,
            semantic,
            ctx,
            machine.symbol,
            state.symbol,
            statement_index,
            target,
            transition.guard,
            region == 1,
            &mut branch_contexts,
            &mut branch_constraints,
        );
        for call in &regions[region] {
            let flow = build_call_flow_fact(
                program,
                borrow,
                proof,
                semantic,
                domains,
                ctx,
                machine,
                state,
                &mut branch_contexts,
                &mut branch_constraints,
                call,
            );
            ctx.control.calls.append_to_span(state_calls, flow);
        }
        if matches!(
            program.statement_table.transition_target(target),
            psi_typed_trees::statement::TransitionTargetNode::SelfTarget
        ) {
            // Self has no argument evaluation or ordinary call record. Retain
            // its actual post-guard contexts for arrival checking. Reuse the
            // immutable fact-reference spans; do not copy or re-seed facts.
            let point = ProgramPoint::TransitionArm {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                statement_index,
                transition_target: target,
            };
            for reference in ctx
                .contexts
                .semantic_context_refs
                .span_or_empty(branch_contexts)
            {
                let context = semantic.contexts.get(reference.context);
                if context.point != point {
                    semantic.append_context(point, context.facts);
                }
            }
        }
        append_state_exit_facts(
            program,
            proof,
            semantic,
            ctx,
            machine.symbol,
            state.symbol,
            target,
            branch_contexts,
            branch_constraints,
        );
    }
    *active_contexts = guard_contexts;
    *active_constraints = guard_constraints;
    guards::append_guard_context(
        program,
        semantic,
        ctx,
        machine.symbol,
        state.symbol,
        statement_index,
        Default::default(),
        transition.guard,
        false,
        active_contexts,
        active_constraints,
    );
}

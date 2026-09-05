use super::*;

mod guards;
pub(super) use guards::append_predicate_context;

pub(super) fn append_state_exit_facts(
    program: &typed_trees::TypedTrees,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    transition_target: typed_trees::statement::TransitionTargetHandle,
    active_contexts: arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: arena::HandleSpan<FlowConstraintRef>,
) -> arena::HandleSpan<FlowExitFact> {
    let mut state_exits = arena::HandleSpan::empty();

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
        let mut ensures_contexts = arena::HandleSpan::empty();
        let mut ensures_constraints = arena::HandleSpan::empty();
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
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    ctx: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    transition: &typed_trees::statement::TableTransition,
    calls: &[BorrowCallFact],
    state_calls: &mut HandleSpan<FlowCallFact>,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let mut execution = super::expression::Execution::new(
        program,
        borrow,
        proof,
        domains,
        machine,
        state,
        statement_index,
        semantic,
        ctx,
        state_calls,
        calls,
        active_contexts,
        active_constraints,
    );
    if let typed_trees::statement::TransitionGuardNode::When(expression) = transition.guard {
        execution.expression(expression, active_contexts, active_constraints);
    }
    // Guards execute on both paths. Their effects survive a missed arm, while
    // calls evaluating one target never run on its sibling continuation.
    let guard_contexts = *active_contexts;
    let guard_constraints = *active_constraints;
    for (region, target) in [(1, transition.target), (2, transition.continuation)] {
        if !target.is_valid() {
            continue;
        }
        let mut branch_contexts = clone_flow_contexts(
            &mut execution.context.contexts.semantic_context_refs,
            guard_contexts,
        );
        let mut branch_constraints = clone_constraint_refs(
            &mut execution.context.contexts.constraint_refs,
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
            region == 1,
            &mut branch_contexts,
            &mut branch_constraints,
        );
        execution.transition_target(
            transition,
            target,
            &mut branch_contexts,
            &mut branch_constraints,
        );
        if matches!(
            program.statement_table.transition_target(target),
            typed_trees::statement::TransitionTargetNode::SelfTarget
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
            for reference in execution
                .context
                .contexts
                .semantic_context_refs
                .span_or_empty(branch_contexts)
            {
                let context = execution.semantic.contexts.get(reference.context);
                if context.point != point {
                    execution.semantic.append_context(point, context.facts);
                }
            }
        }
        append_state_exit_facts(
            program,
            proof,
            execution.semantic,
            execution.context,
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
        execution.semantic,
        execution.context,
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

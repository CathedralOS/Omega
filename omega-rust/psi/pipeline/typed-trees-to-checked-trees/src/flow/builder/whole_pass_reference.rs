//! Pre-worklist reference: every retry rebuilds every state from baseline facts.
//! Kept test-only so scheduling changes can compare complete published evidence.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_whole_pass_reference(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    scalar_expressions: &checked_trees::CheckedScalarExpressionPlans,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> FlowFacts {
    // Reuse the ordinary effect/statement transfer once per input revision.
    // Each pass starts from declaration/proof facts, never a prior pass's
    // derived contexts. Inputs join immediately so source-ordered chains need
    // no pass per edge. A changed input can only weaken, never regain a value.
    let baseline = semantic.clone();
    // Keep declaration-group preparation identical to the current transfer;
    // this reference preserves whole-pass scheduling, not retired lookup work.
    let global_contexts = baseline.context_group_at_point(ProgramPoint::Global);
    let machine_contexts: Vec<_> = program
        .machines()
        .iter()
        .map(|machine| {
            baseline.context_group_at_point(ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            })
        })
        .collect();
    // Symbol preparation depends on the immutable program, not the changing
    // incoming value facts. Prefix origins still resolve at each exact site.
    let call_frames = if borrow.calls.is_empty() {
        None
    } else {
        validation::CallFrameResolver::new(program)
    };
    // These summaries use only program and borrow facts, neither of which
    // changes with the incoming value inputs. Keep first-demand construction
    // lazy, and never carry this table into another flow-build invocation.
    let state_mutation_summary_cache = StateMutationSummaryCache::default();
    let mut inputs = Vec::new();
    // Each state becomes reachable once; each formal can acquire a constant
    // then lose it to unknown once. Include one pass to observe convergence.
    let mut pass_limit = 1 + program
        .machines()
        .iter()
        .map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .map(|state| 1 + 2 * program.state_parameters(state).len())
                .sum::<usize>()
        })
        .sum::<usize>();
    let mut pass = 0;
    while pass < pass_limit {
        if pass >= tests::SWEEP_LIMIT.get() {
            break;
        }
        if pass != 0 {
            *semantic = baseline.clone();
        }
        let mut ctx = FlowBuildContext::new(
            borrow,
            proof,
            semantic,
            scalar_expressions,
            operators,
            exact_integer_casts,
            call_frames.as_ref(),
            &state_mutation_summary_cache,
        );
        ctx.state_value_inputs = inputs;
        for (machine, machine_contexts) in program.machines().iter().zip(&machine_contexts) {
            for state in program.machine_states(machine) {
                build_state_flow_fact(
                    program,
                    borrow,
                    proof,
                    semantic,
                    domains,
                    &mut ctx,
                    machine,
                    state,
                    [global_contexts, *machine_contexts],
                );
            }
        }
        // Each newly reached field contributes one literal and its finite
        // predicate set; parameter qualifications contribute finite membership
        // cells. Subsequent joins only remove these cells.
        pass_limit = pass_limit.saturating_add(ctx.new_state_field_input_height);
        // Inputs arriving before a state's entry was built are already in its
        // contexts. Only a change after that point requires rebuilding flow.
        if !ctx.state_value_inputs_changed_after_build {
            let mut flow = ctx.finish();
            attach_reach_summaries(&mut flow, service_reaches, operational);
            return flow;
        }
        inputs = std::mem::take(&mut ctx.state_value_inputs);
        pass += 1;
    }
    // No provisional input fact survives a nonconvergent graph.
    *semantic = baseline;
    let mut ctx = FlowBuildContext::new(
        borrow,
        proof,
        semantic,
        scalar_expressions,
        operators,
        exact_integer_casts,
        call_frames.as_ref(),
        &state_mutation_summary_cache,
    );
    // Unknown is absorbing: immediate joins during fallback cannot establish
    // a new provisional constant in a state built later in this pass.
    ctx.state_value_inputs = super::state_values::unknown_inputs(program);
    for (machine, machine_contexts) in program.machines().iter().zip(&machine_contexts) {
        for state in program.machine_states(machine) {
            build_state_flow_fact(
                program,
                borrow,
                proof,
                semantic,
                domains,
                &mut ctx,
                machine,
                state,
                [global_contexts, *machine_contexts],
            );
        }
    }
    let mut flow = ctx.finish();
    attach_reach_summaries(&mut flow, service_reaches, operational);
    flow
}

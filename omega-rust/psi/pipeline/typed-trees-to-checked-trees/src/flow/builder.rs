//! Input joins name their changed destination directly. Revisit those states
//! in source order, preserving the original predecessor/evidence encounter order.
//! Each sweep starts from declaration/proof facts: appending a second build into
//! live output would leave stale state contexts visible after an input weakens.
//! Once inputs settle, materialize complete output once in source order so scratch
//! handles never escape and published fact/evidence ordering stays unchanged.
//! The common first-pass fixed point already has complete output and returns it.
//!
//! Whole-output storage is reused only while unpublished; no sweep-local handle
//! survives that replacement boundary. Baseline contents still copy per sweep,
//! including context-point links, while clone_from reuses their allocations.
//! This is not a generation-preserving suffix rollback into a live fact plan.

use super::*;

#[cfg(test)]
pub(super) mod tests;
#[cfg(test)]
mod whole_pass_reference;

#[cfg(test)]
pub(crate) fn build_flow_facts(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    operational: &flow_effects::OperationalPlan,
) -> FlowFacts {
    let service_reaches = validation::infer_service_reaches(program, operational);
    build_flow_facts_with_service_reaches(
        program,
        borrow,
        proof,
        semantic,
        domains,
        operational,
        &service_reaches,
        &Default::default(),
        &Default::default(),
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_flow_facts_with_service_reaches(
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
    #[cfg(test)]
    if tests::WHOLE_PASS_REFERENCE.get() {
        return whole_pass_reference::build_whole_pass_reference(
            program,
            borrow,
            proof,
            semantic,
            domains,
            operational,
            service_reaches,
            scalar_expressions,
            operators,
            exact_integer_casts,
        );
    }

    let baseline = semantic.clone();
    // Declaration contexts do not change during flow. Retain their handles
    // once, in typed-machine order, instead of rediscovering Global/Machine
    // groups for every state on every input pass. The baseline clones preserve
    // these handles. State-input and rebased contexts remain pass-local below.
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
    // Direct assignments also use prefix alias closure when there are no calls.
    // Borrow the same immutable resolver for both statement and call writes.
    let call_frames = validation::CallFrameResolver::new(program);
    // These summaries use only program and borrow facts, neither of which
    // changes with the incoming value inputs. Keep first-demand construction
    // lazy, and never carry this table into another flow-build invocation.
    let state_mutation_summary_cache = StateMutationSummaryCache::default();
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
    let mut complete_pass = true;
    // Each state becomes reachable once; each formal can acquire a constant
    // then lose it to unknown once. Include convergence and final materialization.
    let mut pass_limit = 2 + program
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
        #[cfg(test)]
        if pass >= tests::SWEEP_LIMIT.get() {
            break;
        }
        if pass != 0 {
            ctx.discard_output();
            semantic.clone_from(&baseline);
        }
        for (machine, machine_contexts) in program.machines().iter().zip(&machine_contexts) {
            for state in program.machine_states(machine) {
                if !complete_pass && !ctx.dirty_state_value_inputs.contains(&state.symbol) {
                    continue;
                }
                // Clear before transfer: a self/back edge may dirty this state
                // again. A later destination can still run in this same sweep.
                ctx.dirty_state_value_inputs
                    .retain(|symbol| *symbol != state.symbol);
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
        if ctx.dirty_state_value_inputs.is_empty() {
            if complete_pass {
                let mut flow = ctx.finish();
                attach_reach_summaries(&mut flow, service_reaches, operational);
                return flow;
            }
            complete_pass = true;
        } else {
            if complete_pass && pass != 0 {
                // Final publication must confirm the fixed point; never expose
                // provisional evidence if an untracked dependency changed it.
                break;
            }
            complete_pass = false;
        }
        pass += 1;
    }
    // No provisional input fact survives a nonconvergent graph.
    ctx.discard_output();
    *semantic = baseline;
    // Unknown is absorbing: immediate joins during fallback cannot establish
    // a new provisional constant in a state built later in this pass.
    ctx.state_value_inputs = super::state_values::unknown_inputs(program);
    ctx.dirty_state_value_inputs.clear();
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

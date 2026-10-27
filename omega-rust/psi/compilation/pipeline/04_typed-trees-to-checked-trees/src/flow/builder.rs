//! Input joins name their changed destination directly. Revisit those states
//! in source order, preserving the original predecessor/evidence encounter order.
//! Each sweep starts from declaration/proof facts: appending a second build into
//! live output would leave stale state contexts visible after an input weakens.
//! Once inputs settle, materialize complete output once in source order so scratch
//! handles never escape and published fact/evidence ordering stays unchanged.
//! The common first-pass fixed point already has complete output and returns it.
//!
//! Whole-output storage is reused only while unpublished; no sweep-local handle
//! survives that replacement boundary. Baseline pages and their complete context
//! index are shared before fan-out; each group appends only its own later pages.
//!
//! Value inputs cross state boundaries along transitions and ordinary call
//! argument deliveries, including another machine's entry, so machines
//! joined by such invocations form independent groups. Each group reaches its
//! own fixed point, or its own unknown-input fallback, on a worker over a fork
//! of the shared baseline plan, writing every machine on page
//! `MACHINE_PAGE_BASE + roster position` of each output arena; the owner
//! adopts those pages, so handles and order follow the roster. A group's facts
//! depend only on machines its values reach: a loop in an unrelated machine
//! neither re-sweeps it nor sends it to the fallback.
use crate::flow::FlowBuildContext;
use crate::flow::SharedFlowTables;
use crate::flow::StateMutationSummaryCache;
use crate::flow::attach_reach_summaries;
use crate::flow::build_state_flow_fact;
use checked_trees::{BorrowFacts, DomainFacts, FlowFacts, ProofFacts};
use facts::{FactPlan, ProgramPoint};
use rayon::prelude::*;

/// Page 1 holds the baseline plan; machine pages follow in roster order.
const MACHINE_PAGE_BASE: u32 = 2;

/// The program-fixed inputs every sweep reads.
struct FlowInputs<'program, 'local> {
    program: &'program dyn ::validation::CheckingInput,
    borrow: &'local BorrowFacts,
    proof: &'local ProofFacts,
    domains: &'local DomainFacts,
    baseline: &'local FactPlan,
    global_contexts: facts::FactContextGroup,
    machine_contexts: &'local [facts::FactContextGroup],
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_flow_facts(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    operational: &flow_effects::OperationalPlan,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    scalar_expressions: &checked_trees::CheckedScalarExpressionPlans,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    expression_intervals: &[validation::ExpressionInterval],
    state_mutation_summary_cache: &StateMutationSummaryCache,
    shared_call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> FlowFacts {
    semantic.share_pages();
    let baseline = semantic.clone();
    // Declaration contexts do not change during flow. Retain their handles
    // once, in typed-machine order, instead of rediscovering Global/Machine
    // groups for every state on every input pass. The baseline clones preserve
    // these handles. State-input and rebased contexts remain pass-local below.
    let machine_contexts: Vec<_> = program
        .machines()
        .iter()
        .map(|machine| {
            baseline.context_group_at_point(ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            })
        })
        .collect();
    let inputs = FlowInputs {
        program,
        borrow,
        proof,
        domains,
        baseline: &baseline,
        global_contexts: baseline.context_group_at_point(ProgramPoint::Global),
        machine_contexts: &machine_contexts,
    };
    // Symbol preparation depends on the immutable program, not the changing
    // incoming value facts. Prefix origins still resolve at each exact site.
    // Direct assignments also use prefix alias closure when there are no calls.
    // Borrow the same immutable resolver for both statement and call writes.
    // A caller that already classified frames for this immutable program passes
    // it in; standalone callers still build a local resolver here.
    let mut owned_frames = None;
    let call_frames =
        crate::flow::shared_call_frames_or(shared_call_frames, program, &mut owned_frames);
    // These summaries use only program and borrow facts, neither of which
    // changes with the incoming value inputs. Keep first-demand construction
    // lazy; the check pass owns the table so later consumers reuse it rather
    // than rebuilding identical rows.
    let shared_tables = std::sync::Arc::new(SharedFlowTables::new(program, scalar_expressions));
    let new_build = || {
        FlowBuildContext::with_shared_tables(
            program,
            borrow,
            proof,
            &baseline,
            scalar_expressions,
            operators,
            exact_integer_casts,
            expression_intervals,
            call_frames,
            state_mutation_summary_cache,
            shared_tables.clone(),
        )
    };

    let groups = machine_groups(program, borrow);
    phase_profile::count("05.d.3.b.flow.groups", groups.len() as u64);
    phase_profile::count(
        "05.d.3.b.flow.max_group_machines",
        groups.iter().map(Vec::len).max().unwrap_or(0) as u64,
    );
    let mut flow = build_groups(&inputs, &groups, &new_build, semantic);
    attach_reach_summaries(program, &mut flow, service_reaches, operational);
    flow
}

/// Every group on a worker; the owner adopts each group's pages.
fn build_groups<'plans>(
    inputs: &FlowInputs<'plans, '_>,
    groups: &[Vec<usize>],
    new_build: &(dyn Fn() -> FlowBuildContext<'plans> + Sync),
    semantic: &mut FactPlan,
) -> FlowFacts {
    let groups_built = validation::checking_thread_pool().install(|| {
        groups
            .par_iter()
            .map_init(
                || (inputs.baseline.clone(), new_build()),
                |(worker_semantic, build), group| {
                    worker_semantic.clone_from(inputs.baseline);
                    build_group(inputs, worker_semantic, build, group);
                    (
                        build.take_output_pages(),
                        worker_semantic.take_additional_pages(),
                    )
                },
            )
            .collect::<Vec<_>>()
    });
    let mut flow = FlowFacts::default();
    for (pages, semantic_pages) in groups_built {
        flow.contexts.adopt_additional_pages(pages.contexts);
        flow.invalidations
            .adopt_additional_pages(pages.invalidations);
        flow.borrow_lifetimes
            .adopt_additional_pages(pages.borrow_lifetimes);
        flow.boundaries.adopt_additional_pages(pages.boundaries);
        flow.control.adopt_additional_pages(pages.control);
        semantic.adopt_additional_pages(semantic_pages);
    }
    flow
}

/// One group's fixed point, or its unknown-input fallback.
fn build_group<'plans>(
    inputs: &FlowInputs<'plans, '_>,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'plans>,
    machines: &[usize],
) {
    if converge(inputs, semantic, build, machines) {
        return;
    }
    // No provisional input fact survives a nonconvergent graph.
    build.discard_output();
    semantic.clone_from(inputs.baseline);
    // Unknown is absorbing: immediate joins during fallback cannot establish
    // a new provisional constant in a state built later in this pass.
    build.state_value_inputs = super::state_values::unknown_inputs(inputs.program, machines);
    build.dirty_state_value_inputs.clear();
    sweep(inputs, semantic, build, machines, true);
}

/// Sweep `machines` until their value inputs settle, then confirm with one
/// complete pass. `false` when the bound is exhausted or confirmation fails.
fn converge<'plans>(
    inputs: &FlowInputs<'plans, '_>,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'plans>,
    machines: &[usize],
) -> bool {
    let program = inputs.program;
    let mut complete_pass = true;
    // Each state becomes reachable once; each formal can acquire a constant
    // then lose it to unknown once. Include convergence and final materialization.
    let mut pass_limit = 2 + machines
        .iter()
        .map(|position| {
            program
                .machine_states(&program.machines()[*position])
                .iter()
                .map(|state| 1 + 2 * program.state_parameters(state).len())
                .sum::<usize>()
        })
        .sum::<usize>();
    let mut pass = 0;
    while pass < pass_limit {
        phase_profile::count("05.d.3.b.flow.inner_passes", 1);
        if pass != 0 {
            build.discard_output();
            semantic.clone_from(inputs.baseline);
        }
        sweep(inputs, semantic, build, machines, complete_pass);
        // Each newly reached field contributes one literal and its finite
        // predicate set; parameter qualifications contribute finite membership
        // cells. Scalar parameter intervals additionally traverse a finite
        // outward threshold set; their allowance is registered at discovery.
        pass_limit = pass_limit.saturating_add(build.new_state_field_input_height);
        if build.dirty_state_value_inputs.is_empty() {
            if complete_pass {
                return true;
            }
            complete_pass = true;
        } else {
            if complete_pass && pass != 0 {
                // Final publication must confirm the fixed point; never expose
                // provisional evidence if an untracked dependency changed it.
                return false;
            }
            complete_pass = false;
        }
        pass += 1;
    }
    false
}

/// One pass over `machines`: every state when `complete`, otherwise only the
/// states whose value inputs changed. Each visited machine writes its own page.
fn sweep<'plans>(
    inputs: &FlowInputs<'plans, '_>,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'plans>,
    machines: &[usize],
    complete: bool,
) {
    for &position in machines {
        let machine = &inputs.program.machines()[position];
        let mut paged = false;
        for state in inputs.program.machine_states(machine) {
            if !complete && !build.dirty_state_value_inputs.contains(&state.symbol) {
                continue;
            }
            if !paged {
                let page = MACHINE_PAGE_BASE
                    + u32::try_from(position).expect("machine roster exceeds page identities");
                build.begin_output_page(page);
                semantic.begin_page(page);
                paged = true;
            }
            phase_profile::count("05.d.3.b.flow.states_visited", 1);
            // Clear before transfer: a self/back edge may dirty this state
            // again. A later destination can still run in this same sweep.
            build
                .dirty_state_value_inputs
                .retain(|symbol| *symbol != state.symbol);
            build_state_flow_fact(
                inputs.program,
                inputs.borrow,
                inputs.proof,
                semantic,
                inputs.domains,
                build,
                machine,
                state,
                [inputs.global_contexts, inputs.machine_contexts[position]],
            );
        }
    }
}

/// Roster positions grouped by an invocation that writes another machine's
/// `state_value_inputs`: a call to a non-entry state of another machine, or an
/// extracted lexical region and its origin machine. Ordinary machine-entry
/// calls check the callee under its declared premises and do not couple the
/// two machines' value-input fixed points. Groups keep roster order, inside and
/// across groups, so adopted pages publish in that order.
fn machine_groups(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
) -> Vec<Vec<usize>> {
    fn root(owner: &mut [usize], mut position: usize) -> usize {
        while owner[position] != position {
            owner[position] = owner[owner[position]];
            position = owner[position];
        }
        position
    }
    let machines = program.machines();
    // The same symbol-to-position resolution `record_invocation` uses: a later
    // duplicate state symbol replaces an earlier one.
    let mut state_locations = symbols::SymbolKeyMap::default();
    for (machine_position, machine) in machines.iter().enumerate() {
        for (state_position, state) in program.machine_states(machine).iter().enumerate() {
            state_locations.insert(state.symbol, (machine_position, state_position));
        }
    }
    let mut owner = (0..machines.len()).collect::<Vec<_>>();
    for (position, machine) in machines.iter().enumerate() {
        if let Some(origin) = machine.lexical_region_origin
            && let Some(origin_position) = program.machine_position_by_symbol(origin.machine)
            && origin_position != position
        {
            let (left, right) = (
                root(&mut owner, position),
                root(&mut owner, origin_position),
            );
            owner[left.max(right)] = left.min(right);
        }
    }
    for (_, row) in borrow.states.iter() {
        let Some(caller) = program.machine_position_by_symbol(row.machine_symbol) else {
            continue;
        };
        for call in borrow.calls.span_or_empty(row.calls) {
            let Some(&(callee, state_position)) = state_locations.get(&call.target_symbol) else {
                continue;
            };
            let callee_machine = &machines[callee];
            if callee_machine.lexical_region_origin.is_none()
                && super::state_values::entry_state_index(program, callee_machine)
                    == Some(state_position)
            {
                continue;
            }
            // Keep every authored family body in the same dependency group.
            // Each body is checked under its declared premises; grouping grants
            // no selected-case or scalar assumption from a caller.
            let mut callees = vec![callee];
            let machine_symbol = machines[callee].symbol;
            if let Some(family) = program.target_machine_families.iter().find(|family| {
                family.canonical == machine_symbol
                    || family
                        .members
                        .iter()
                        .any(|member| member.machine == machine_symbol)
            }) {
                for member in std::iter::once(family.canonical)
                    .chain(family.members.iter().map(|member| member.machine))
                {
                    if let Some(position) = program.machine_position_by_symbol(member) {
                        crate::collections::push_unique(&mut callees, position);
                    }
                }
            }
            for callee in callees {
                if callee != caller {
                    let (left, right) = (root(&mut owner, caller), root(&mut owner, callee));
                    owner[left.max(right)] = left.min(right);
                }
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut group_of_root = vec![usize::MAX; machines.len()];
    for position in 0..machines.len() {
        let group_root = root(&mut owner, position);
        if group_of_root[group_root] == usize::MAX {
            group_of_root[group_root] = groups.len();
            groups.push(Vec::new());
        }
        groups[group_of_root[group_root]].push(position);
    }
    groups
}

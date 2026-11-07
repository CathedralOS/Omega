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
//!
//! Values only flow from caller to callee, so a group whose callers share a
//! private helper is still serial at the helper. Each part of the group that
//! no other part calls first converges alone on its own worker, recording
//! every input delivery it makes. The group then runs its usual fixed point,
//! building the remaining machines and replaying those deliveries at the
//! visits where the serial run would make them; any visit that does not match
//! the recording sends the whole group back to the serial build.
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
    let shared_tables = std::sync::Arc::new(SharedFlowTables::new(
        program,
        scalar_expressions,
        call_frames,
    ));
    let new_build = || {
        FlowBuildContext::with_shared_tables(
            program,
            borrow,
            proof,
            &baseline,
            scalar_expressions,
            operators,
            call_frames,
            state_mutation_summary_cache,
            shared_tables.clone(),
        )
    };

    let groups = machine_groups(program, borrow);
    let mut flow = build_groups(&inputs, &groups, &new_build, semantic);
    attach_reach_summaries(program, &mut flow, service_reaches, operational);
    flow
}

/// Every group on a worker; the owner adopts each group's pages.
fn build_groups<'plans>(
    inputs: &FlowInputs<'plans, '_>,
    groups: &[MachineGroup],
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
                    facts::note_declaration_page_fork(worker_semantic, inputs.baseline);
                    if group.independent_parts.len() >= 2
                        && let Some(pages) =
                            build_split_group(inputs, new_build, worker_semantic, build, group)
                    {
                        return pages;
                    }
                    build_group(inputs, worker_semantic, build, &group.machines);
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
        adopt_flow_pages(&mut flow, pages);
        semantic.adopt_additional_pages(semantic_pages);
    }
    flow
}

fn adopt_flow_pages(flow: &mut FlowFacts, pages: FlowFacts) {
    flow.contexts.adopt_additional_pages(pages.contexts);
    flow.invalidations
        .adopt_additional_pages(pages.invalidations);
    flow.borrow_lifetimes
        .adopt_additional_pages(pages.borrow_lifetimes);
    flow.boundaries.adopt_additional_pages(pages.boundaries);
    flow.control.adopt_additional_pages(pages.control);
}

/// One independent part's solo visits, replayed into its group's run.
struct PartReplay {
    machines: Vec<usize>,
    visits: Vec<super::state_values::RecordedVisit>,
    /// The solo run's last pass, which was complete. Every later complete
    /// pass of the group revisits the part from the same settled inputs.
    final_pass: usize,
    final_start: usize,
    cursor: usize,
}

/// Each independent part converges alone on its own worker, recording the
/// value-input deliveries of every state visit. The group then runs its own
/// fixed point, building only the remaining machines and replaying each
/// part's deliveries where its visits fall, so every input row, dirty state,
/// pass and bound evolves as in the serial group run. `None` when a part or
/// the replay does not reproduce that run; the caller then builds serially.
fn build_split_group<'plans>(
    inputs: &FlowInputs<'plans, '_>,
    new_build: &(dyn Fn() -> FlowBuildContext<'plans> + Sync),
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'plans>,
    group: &MachineGroup,
) -> Option<(FlowFacts, FactPlan)> {
    let parts = group
        .independent_parts
        .par_iter()
        .map(|part| {
            let mut part_semantic = inputs.baseline.clone();
            let mut part_build = new_build();
            // Seeds are not visits: the serial run applies them to these rows
            // before its first pass, and the group run below seeds them again
            // itself, so they are applied here and never recorded. A seed into
            // another part's row would change that part's inputs; build serially.
            super::state_values::seed_contract_inputs(inputs.program, &mut part_build, part);
            let seeds_stay_in_part = part_build.dirty_state_value_inputs.iter().all(|state| {
                part_build
                    .state_location(inputs.program, *state)
                    .is_some_and(|(machine, _)| part.binary_search(&machine).is_ok())
            });
            if !seeds_stay_in_part {
                return None;
            }
            part_build.input_recording = Some(super::state_values::InputRecording {
                machines: part.clone(),
                visits: Vec::new(),
            });
            let converged = converge(inputs, &mut part_semantic, &mut part_build, part, &mut []);
            let visits = part_build.input_recording.take()?.visits;
            let final_pass = converged?;
            let final_start = visits
                .iter()
                .position(|visit| visit.pass == final_pass)
                .unwrap_or(visits.len());
            Some((
                PartReplay {
                    machines: part.clone(),
                    visits,
                    final_pass,
                    final_start,
                    cursor: 0,
                },
                part_build.take_output_pages(),
                part_semantic.take_additional_pages(),
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let mut replays = Vec::with_capacity(parts.len());
    let mut part_pages = Vec::with_capacity(parts.len());
    for (replay, pages, semantic_pages) in parts {
        replays.push(replay);
        part_pages.push((pages, semantic_pages));
    }
    // From here on only a part may write its own rows; seeds are attributed to
    // the machine that owns them, in the same order the serial build seeds.
    build.replay_guard = Some(super::state_values::ReplayGuard::new(
        inputs.program.machines().len(),
        &group.independent_parts,
    ));
    for &position in &group.machines {
        if let Some(guard) = build.replay_guard.as_mut() {
            guard.set_writer(position);
        }
        super::state_values::seed_contract_inputs(inputs.program, build, &[position]);
    }
    let replayed = replay_guard_holds(build)
        && converge(inputs, semantic, build, &group.machines, &mut replays).is_some()
        && replays
            .iter()
            .all(|replay| replay.cursor == replay.visits.len());
    build.replay_guard = None;
    if !replayed {
        build.take_output_pages();
        semantic.clone_from(inputs.baseline);
        return None;
    }
    // Merge before the owner adopts: one adoption then meets every page of
    // the group in generation order, as the serial group's adoption does.
    let mut pages = build.take_output_pages();
    let mut semantic_pages = semantic.take_additional_pages();
    for (part, part_semantic) in part_pages {
        adopt_flow_pages(&mut pages, part);
        semantic_pages.adopt_additional_pages(part_semantic);
    }
    Some((pages, semantic_pages))
}

/// One group's fixed point, or its unknown-input fallback.
fn build_group<'plans>(
    inputs: &FlowInputs<'plans, '_>,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'plans>,
    machines: &[usize],
) {
    super::state_values::seed_contract_inputs(inputs.program, build, machines);
    if converge(inputs, semantic, build, machines, &mut []).is_some() {
        return;
    }
    // No provisional input fact survives a nonconvergent graph.
    build.discard_output();
    semantic.clone_from(inputs.baseline);
    // Unknown is absorbing: immediate joins during fallback cannot establish
    // a new provisional constant in a state built later in this pass.
    build.replace_state_value_inputs(super::state_values::unknown_inputs(
        inputs.program,
        machines,
    ));
    build.dirty_state_value_inputs.clear();
    sweep(inputs, semantic, build, machines, true, 0, &mut []);
}

/// Sweep `machines` until their value inputs settle, then confirm with one
/// complete pass, returning that pass. `None` when the bound is exhausted,
/// confirmation fails, or a replayed part's visits diverge.
fn converge<'plans>(
    inputs: &FlowInputs<'plans, '_>,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'plans>,
    machines: &[usize],
    replays: &mut [PartReplay],
) -> Option<usize> {
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
        if pass != 0 {
            build.discard_output();
            semantic.clone_from(inputs.baseline);
        }
        if !sweep(
            inputs,
            semantic,
            build,
            machines,
            complete_pass,
            pass,
            replays,
        ) || !replay_guard_holds(build)
        {
            return None;
        }
        // Each newly reached field contributes one literal and its finite
        // predicate set; parameter qualifications contribute finite membership
        // cells. Scalar parameter intervals additionally traverse a finite
        // outward threshold set; their allowance is registered at discovery.
        pass_limit = pass_limit.saturating_add(build.new_state_field_input_height);
        if build.dirty_state_value_inputs.is_empty() {
            if complete_pass {
                return Some(pass);
            }
            complete_pass = true;
        } else {
            if complete_pass && pass != 0 {
                // Final publication must confirm the fixed point; never expose
                // provisional evidence if an untracked dependency changed it.
                return None;
            }
            complete_pass = false;
        }
        pass += 1;
    }
    None
}

/// One pass over `machines`: every state when `complete`, otherwise only the
/// states whose value inputs changed. Each visited machine writes its own page.
/// A replayed part's state repeats its recorded deliveries instead of being
/// built; `false` when that visit is not the next one the part recorded.
#[allow(clippy::too_many_arguments)]
fn sweep<'plans>(
    inputs: &FlowInputs<'plans, '_>,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'plans>,
    machines: &[usize],
    complete: bool,
    pass: usize,
    replays: &mut [PartReplay],
) -> bool {
    // A later complete pass revisits each part from its settled inputs, which
    // is the part's own last pass.
    let expected_pass = if complete && pass != 0 {
        for replay in replays.iter_mut() {
            replay.cursor = replay.final_start;
        }
        None
    } else {
        Some(pass)
    };
    for &position in machines {
        let machine = &inputs.program.machines()[position];
        if let Some(guard) = build.replay_guard.as_mut() {
            guard.set_writer(position);
        }
        let mut replay = replays
            .iter_mut()
            .find(|replay| replay.machines.binary_search(&position).is_ok());
        let mut paged = false;
        for state in inputs.program.machine_states(machine) {
            if !complete && !build.dirty_state_value_inputs.contains(&state.symbol) {
                continue;
            }
            if let Some(replay) = replay.as_deref_mut() {
                build
                    .dirty_state_value_inputs
                    .retain(|symbol| *symbol != state.symbol);
                let expected = expected_pass.unwrap_or(replay.final_pass);
                let Some(visit) = replay
                    .visits
                    .get(replay.cursor)
                    .filter(|visit| visit.state == state.symbol && visit.pass == expected)
                else {
                    return false;
                };
                for delivery in &visit.deliveries {
                    if !super::state_values::replay_delivery(inputs.program, build, delivery) {
                        return false;
                    }
                }
                replay.cursor += 1;
                continue;
            }
            if !paged {
                let page = MACHINE_PAGE_BASE
                    + u32::try_from(position).expect("machine roster exceeds page identities");
                build.begin_output_page(page);
                semantic.begin_page(page);
                paged = true;
            }
            // Clear before transfer: a self/back edge may dirty this state
            // again. A later destination can still run in this same sweep.
            build
                .dirty_state_value_inputs
                .retain(|symbol| *symbol != state.symbol);
            if let Some(recording) = build.input_recording.as_mut() {
                recording.visits.push(super::state_values::RecordedVisit {
                    state: state.symbol,
                    pass,
                    deliveries: Vec::new(),
                });
            }
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
    true
}

/// Roster positions grouped by an invocation that writes another machine's
/// `state_value_inputs`: a non-entry call, an extracted or nested machine, or
/// a private entry whose caller roster supplies the entry facts. A public
/// entry stays on its declared premises and does not join the caller's group.
///
/// Within a group, a part is a machine with its lexical relatives, the family
/// bodies called together with it, and every part that only it calls. A part
/// no other part calls receives only its own deliveries, so it converges
/// alone and its group replays it (`independent_parts`): each hosted target's
/// `plan` body is such a part, beside the entry-shape helpers they share.
fn machine_groups(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
) -> Vec<MachineGroup> {
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
        let lexical_parent = machine
            .lexical_region_origin
            .map(|origin| origin.machine)
            .or_else(|| {
                let scope = program.declaration_scope_parent(machine.symbol);
                scope
                    .is_valid()
                    .then(|| program.machine_declaration_for_lexical_scope(scope))
                    .flatten()
                    .map(|declaring| declaring.symbol)
            });
        if let Some(origin_machine) = lexical_parent
            && let Some(origin_position) = program.machine_position_by_symbol(origin_machine)
            && origin_position != position
        {
            let (left, right) = (
                root(&mut owner, position),
                root(&mut owner, origin_position),
            );
            owner[left.max(right)] = left.min(right);
        }
    }
    let mut part_owner = owner.clone();
    let mut calls = Vec::new();
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
                && !super::state_values::lexically_nested_machine(program, callee_machine.symbol)
                && super::state_values::entry_state_index(program, callee_machine)
                    == Some(state_position)
                && !super::state_values::caller_roster_supplies_entry(program, callee_machine)
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
                        crate::shared::collections::push_unique(&mut callees, position);
                    }
                }
            }
            for &member in &callees[1..] {
                let (left, right) = (
                    root(&mut part_owner, callees[0]),
                    root(&mut part_owner, member),
                );
                part_owner[left.max(right)] = left.min(right);
            }
            for callee in callees {
                if callee != caller {
                    calls.push((caller, callee));
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
    // A part every one of whose callers lies in one other part belongs with
    // that caller: a policy wrapper takes the body it alone calls. Merge to a
    // fixed point; a helper called from two targets stays its own part.
    const UNCALLED: usize = usize::MAX;
    const SHARED: usize = usize::MAX - 1;
    let mut sole_caller = vec![UNCALLED; machines.len()];
    loop {
        sole_caller.fill(UNCALLED);
        for &(caller, callee) in &calls {
            let (caller, callee) = (root(&mut part_owner, caller), root(&mut part_owner, callee));
            if caller != callee {
                sole_caller[callee] = match sole_caller[callee] {
                    UNCALLED => caller,
                    sole if sole == caller => caller,
                    _ => SHARED,
                };
            }
        }
        let mut merged = false;
        for (callee, &caller) in sole_caller.iter().enumerate() {
            if caller < SHARED {
                let (left, right) = (root(&mut part_owner, callee), root(&mut part_owner, caller));
                if left != right {
                    part_owner[left.max(right)] = left.min(right);
                    merged = true;
                }
            }
        }
        if !merged {
            break;
        }
    }
    // Part roots are roster positions, so a dense table marks the called ones.
    let mut called = vec![false; machines.len()];
    for (caller, callee) in calls {
        let callee_part = root(&mut part_owner, callee);
        if root(&mut part_owner, caller) != callee_part {
            called[callee_part] = true;
        }
    }
    let mut part_of_root = vec![usize::MAX; machines.len()];
    groups
        .into_iter()
        .map(|machines| {
            let mut parts: Vec<(usize, Vec<usize>)> = Vec::new();
            for &position in &machines {
                let part = root(&mut part_owner, position);
                if part_of_root[part] == usize::MAX {
                    part_of_root[part] = parts.len();
                    parts.push((part, Vec::new()));
                }
                parts[part_of_root[part]].1.push(position);
            }
            let independent_parts = if parts.len() > 1 {
                parts
                    .into_iter()
                    .filter(|(root, _)| !called[*root])
                    .map(|(_, members)| members)
                    .collect()
            } else {
                Vec::new()
            };
            MachineGroup {
                machines,
                independent_parts,
            }
        })
        .collect()
}

/// One flow dependency group in roster order, with the parts of it that
/// converge alone, each in roster order.
struct MachineGroup {
    machines: Vec<usize>,
    independent_parts: Vec<Vec<usize>>,
}

/// No delivery outside a replayed part reached one of its rows.
fn replay_guard_holds(build: &FlowBuildContext<'_>) -> bool {
    build
        .replay_guard
        .as_ref()
        .is_none_or(super::state_values::ReplayGuard::holds)
}

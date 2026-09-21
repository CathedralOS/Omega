//! Default-domain writes, cross-state facts, and invariant windows.
//!
//! A store may open a window, but consumption requires its `where` facts to
//! hold again; explicit crashes retain the open data identities as damage
//! evidence. State walks transport three distinct facts: definite establishment
//! by intersection, literal field valuations by agreement over reached exits,
//! and open windows by union. Reachability and semantic map equality must stay
//! stable across fixed-point publication; see `StateExit` and the valuation meet.
//!
//! Machine-owned fields are born zero only at a non-reentrant boot entry.
//! Other entries use transported values, never an invented zero. Whole-place
//! literals reseed fields; resolved calls invalidate valuations overlapping
//! their may-write paths, while opaque calls retain the whole-state fence.
//!
//! This file owns the write analysis and the state walk.
//! `assignment_windows.rs` handles assignments and open windows and
//! `data_reads.rs` validates data reads.

mod assignment_windows;
mod call_summaries;
mod data_reads;
mod place_queries;
mod state_flow;
mod symbolic_values;
#[cfg(test)]
mod tests;
mod where_fact_intervals;

pub(crate) use where_fact_intervals::where_fact_interval;

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::data::DataDefinition;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

use crate::proof_contracts::default_domains::assignment_windows::handle_assignment;
use crate::proof_contracts::default_domains::assignment_windows::preserve_proven_establishment;
use crate::proof_contracts::default_domains::assignment_windows::refuse_open_windows;
use crate::proof_contracts::default_domains::data_reads::attached_value_established;
use crate::proof_contracts::default_domains::data_reads::scan_statement_reads;
use call_summaries::{collect_call_summaries, machine_symbol_for_state};
use place_queries::{
    is_self_rooted, transition_evaluated_expressions, write_target_index_expressions,
};
use state_flow::{PlaceValuation, canonicalize_valuations, meet_valuations, state_edges};
use symbolic_values::{SymbolicValue, expression_contains_call};

type InvariantWindow = (String, String, symbols::SymbolHandle);

/// Reachability belongs to the entry that produced this exit, not to the
/// entries being updated during the following meet. Otherwise a newly reached
/// state publishes its old, unvisited walk as known-empty and cyclic constants
/// can alternate with unknown forever.
struct StateExit {
    established: Vec<String>,
    valuations: Vec<PlaceValuation>,
    windows: Vec<InvariantWindow>,
    was_reached: bool,
}

/// Source-independent evidence that one explicit crash occurs while at least
/// one default-domain invariant window is open. The place spelling remains a
/// validator diagnostic concern; checked damage evidence retains only the
/// invariant-bearing data identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenInvariantCrashSite {
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
    open_data: Vec<symbols::SymbolHandle>,
}

impl OpenInvariantCrashSite {
    pub const fn machine(&self) -> symbols::SymbolHandle {
        self.machine
    }

    pub const fn state(&self) -> symbols::SymbolHandle {
        self.state
    }

    pub const fn statement_ordinal(&self) -> u32 {
        self.statement_ordinal
    }

    pub fn open_data(&self) -> &[symbols::SymbolHandle] {
        &self.open_data
    }
}

pub(crate) fn validate_default_domain_writes(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    analyze_default_domain_writes(program, diagnostics, &mut Vec::new());
}

pub fn build_open_invariant_crash_sites(program: &TypedTrees) -> Vec<OpenInvariantCrashSite> {
    let mut diagnostics = Vec::new();
    let mut sites = Vec::new();
    analyze_default_domain_writes(program, &mut diagnostics, &mut sites);
    sites.sort_by_key(|site| {
        (
            site.machine.arena_index(),
            site.machine.generation(),
            site.state.arena_index(),
            site.state.generation(),
            site.statement_ordinal,
        )
    });
    sites.dedup();
    sites
}

fn analyze_default_domain_writes(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
    crash_sites: &mut Vec<OpenInvariantCrashSite>,
) {
    // Every state walk consults the same immutable call-resolution catalog.
    // Building it per fixpoint visit repeated the whole-program symbol scan
    // hundreds of times for larger programs without changing any result.
    let call_frames = crate::machine_calls::calls::CallFrameResolver::new(program);

    // R2 rung 3 slice 11 (+ multi-state extension): per-machine
    // establishment SUMMARIES -- the self places a callee DEFINITELY
    // establishes, walked with born_zero=false (a callee runs at arbitrary
    // times) and no nested summaries (conservative). Establishment is
    // globally monotone, so a call can only ADD it at the call site.
    // Multi-state callees run the same must-fixpoint the main pass uses
    // (intersection meet over predecessors), and the summary INTERSECTS the
    // exit sets of the TERMINAL states (no outgoing transition -- the only
    // places the callee can return from); a dispatch state's own exit is
    // not a return point. No terminal states (a cyclic graph) summarizes
    // as nothing -- conservative.
    let mut summaries: Vec<(symbols::SymbolHandle, Vec<String>)> = Vec::new();
    let mut throwaway = Vec::new();
    let mut throwaway_crash_sites = Vec::new();
    for machine in program.machines() {
        let states = program.machine_states(machine);
        if states.is_empty() {
            continue;
        }
        let (exits, terminal): (Vec<Vec<String>>, Vec<usize>) = if states.len() == 1 {
            (
                vec![
                    walk_state(
                        program,
                        call_frames.as_ref(),
                        machine,
                        &states[0],
                        &[],
                        &[],
                        &[],
                        &[],
                        false,
                        true,
                        &mut throwaway,
                        &mut throwaway_crash_sites,
                        false,
                    )
                    .0,
                ],
                vec![0],
            )
        } else {
            let edges = state_edges(program, states);
            let mut entry: Vec<Vec<String>> = vec![Vec::new(); states.len()];
            let exits = loop {
                let exits: Vec<Vec<String>> = states
                    .iter()
                    .enumerate()
                    .map(|(index, state)| {
                        walk_state(
                            program,
                            call_frames.as_ref(),
                            machine,
                            state,
                            &entry[index],
                            &[],
                            &[],
                            &[],
                            false,
                            true,
                            &mut throwaway,
                            &mut throwaway_crash_sites,
                            false,
                        )
                        .0
                    })
                    .collect();
                let mut changed = false;
                for (index, current_entry) in entry.iter_mut().enumerate().skip(1) {
                    let predecessors: Vec<usize> = edges
                        .iter()
                        .filter(|(_, to)| *to == index)
                        .map(|(from, _)| *from)
                        .collect();
                    if predecessors.is_empty() {
                        continue;
                    }
                    let mut meet: Option<Vec<String>> = None;
                    for predecessor in &predecessors {
                        let exit = &exits[*predecessor];
                        meet = Some(match meet {
                            None => exit.clone(),
                            Some(current) => current
                                .into_iter()
                                .filter(|place| exit.contains(place))
                                .collect(),
                        });
                    }
                    let meet = meet.unwrap_or_default();
                    if meet != *current_entry {
                        *current_entry = meet;
                        changed = true;
                    }
                }
                if !changed {
                    break exits;
                }
            };
            let terminal: Vec<usize> = (0..states.len())
                .filter(|index| !edges.iter().any(|(from, _)| from == index))
                .collect();
            (exits, terminal)
        };
        if terminal.is_empty() {
            continue;
        }
        let mut definite: Option<Vec<String>> = None;
        for index in &terminal {
            let exit = &exits[*index];
            definite = Some(match definite {
                None => exit.clone(),
                Some(current) => current
                    .into_iter()
                    .filter(|place| exit.contains(place))
                    .collect(),
            });
        }
        let self_rooted: Vec<String> = definite
            .unwrap_or_default()
            .into_iter()
            .filter(|spelling| is_self_rooted(spelling))
            .collect();
        if !self_rooted.is_empty() {
            summaries.push((machine.symbol, self_rooted));
        }
    }

    for machine in program.machines() {
        let states = program.machine_states(machine);
        // Bodyless machines (boundary/requirement declarations) own no
        // states -- nothing to walk.
        if states.is_empty() {
            continue;
        }
        // R2 rung 3 slice 3: CROSS-STATE establishment. Establishment is
        // globally monotone in the strict model (every accepted write
        // anywhere re-proves the domain), so a MUST analysis over the
        // state graph is sound: established at entry of S = established at
        // exit of EVERY predecessor. Bottom-start iteration converges to
        // the LEAST fixpoint -- an UNDER-approximation (loop-carried
        // establishment stays conservative), which only over-refuses.
        let edges = state_edges(program, states);
        // R2 rung 3 slice 4 (SOUNDNESS): untracked fields read the born
        // zero ONLY in the boot state when nothing can re-enter it --
        // machine-owned fields persist, so in any other state an untracked
        // field may hold a prior value and must fold as UNKNOWN (poison ->
        // directed refusal; cross-state valuation transport is the
        // precision rung).
        let born_zero = |index: usize| index == 0 && !edges.iter().any(|(_, to)| *to == 0);
        // R2 rung 3 slice 5: the combined MUST fixpoint -- establishment
        // (as slice 3) and per-place field VALUATIONS (Kildall constant
        // propagation: non-boot entries start TOP/unvisited; meet keeps a
        // field only when every visited predecessor exits it with the SAME
        // literal; establishment survives calls, valuations do not).
        let mut entry_established: Vec<Vec<String>> = vec![Vec::new(); states.len()];
        let mut entry_valuations: Vec<Option<Vec<PlaceValuation>>> = vec![None; states.len()];
        entry_valuations[0] = Some(Vec::new());
        // WINDOW TRANSPORT: open windows at each state's entry -- the
        // MAY-union of predecessor exits (an obligation from ANY path in).
        let mut entry_windows: Vec<Vec<InvariantWindow>> = vec![Vec::new(); states.len()];
        // A TERMINAL state (no outgoing transition) is where the machine
        // returns: its exit is a hard consumption point for open windows.
        let is_terminal = |index: usize| !edges.iter().any(|(from, _)| *from == index);
        loop {
            let mut changed = false;
            let exits: Vec<StateExit> = states
                .iter()
                .enumerate()
                .map(|(index, state)| {
                    let (established, valuations, windows) = walk_state(
                        program,
                        call_frames.as_ref(),
                        machine,
                        state,
                        &entry_established[index],
                        entry_valuations[index].as_deref().unwrap_or(&[]),
                        &entry_windows[index],
                        &summaries,
                        born_zero(index),
                        is_terminal(index),
                        &mut throwaway,
                        &mut throwaway_crash_sites,
                        false,
                    );
                    StateExit {
                        established,
                        valuations,
                        windows,
                        was_reached: entry_valuations[index].is_some(),
                    }
                })
                .collect();
            for index in 1..states.len() {
                let predecessors: Vec<usize> = edges
                    .iter()
                    .filter(|(_, to)| *to == index)
                    .map(|(from, _)| *from)
                    .collect();
                if predecessors.is_empty() {
                    continue;
                }
                // Establishment meet (intersection over ALL predecessors).
                let mut established_meet: Option<Vec<String>> = None;
                for predecessor in &predecessors {
                    let exit = &exits[*predecessor].established;
                    established_meet = Some(match established_meet {
                        None => exit.clone(),
                        Some(current) => current
                            .into_iter()
                            .filter(|place| exit.contains(place))
                            .collect(),
                    });
                }
                let established_meet = established_meet.unwrap_or_default();
                if established_meet != entry_established[index] {
                    entry_established[index] = established_meet;
                    changed = true;
                }
                // Window MAY-union: open from ANY predecessor -> open here.
                let mut window_union: Vec<InvariantWindow> = Vec::new();
                for predecessor in &predecessors {
                    for window in &exits[*predecessor].windows {
                        if !window_union.contains(window) {
                            window_union.push(window.clone());
                        }
                    }
                }
                window_union.sort_by(|left, right| {
                    (&left.0, &left.1, left.2.arena_index(), left.2.generation()).cmp(&(
                        &right.0,
                        &right.1,
                        right.2.arena_index(),
                        right.2.generation(),
                    ))
                });
                if window_union != entry_windows[index] {
                    entry_windows[index] = window_union;
                    changed = true;
                }
                // Valuation meet (over VISITED predecessors only -- the
                // Kildall optimism; unvisited preds resolve as iteration
                // reaches them, only ever REMOVING knowledge).
                let visited: Vec<usize> = predecessors
                    .iter()
                    .copied()
                    .filter(|predecessor| exits[*predecessor].was_reached)
                    .collect();
                if visited.is_empty() {
                    continue;
                }
                let mut valuation_meet: Option<Vec<PlaceValuation>> = None;
                for predecessor in visited {
                    let exit = &exits[predecessor].valuations;
                    valuation_meet = Some(match valuation_meet {
                        None => exit.clone(),
                        Some(current) => meet_valuations(&current, exit),
                    });
                }
                let mut valuation_meet = valuation_meet.unwrap_or_default();
                // These vectors represent maps. Statement/constructor order
                // must not count as a changed fact at the fixed-point boundary,
                // including when there is only one predecessor and no meet.
                canonicalize_valuations(&mut valuation_meet);
                if entry_valuations[index].as_ref() != Some(&valuation_meet) {
                    entry_valuations[index] = Some(valuation_meet);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for (index, state) in states.iter().enumerate() {
            walk_state(
                program,
                call_frames.as_ref(),
                machine,
                state,
                &entry_established[index],
                entry_valuations[index].as_deref().unwrap_or(&[]),
                &entry_windows[index],
                &summaries,
                born_zero(index),
                is_terminal(index),
                diagnostics,
                crash_sites,
                true,
            );
        }
    }
}

/// One tracked place: its rendered spelling, its data definition, and the
/// per-field valuation (`None` value = written with a non-literal).
struct TrackedPlace<'program> {
    spelling: String,
    definition: &'program DataDefinition,
    fields: Vec<(String, Option<i128>)>,
    symbols: Vec<(String, SymbolicValue)>,
    measures: Vec<(String, Option<i128>, Option<i128>)>,
    /// R2 rung 3 slice 2: the ACCESS GATE. A `zero_gated` place starts
    /// UNESTABLISHED (its zero violates the domain); a proven whole-place
    /// literal or an accepted constrained write establishes it (every
    /// accepted write leaves the facts true). Reads before establishment
    /// refuse. Zero-satisfying places are born established.
    established: bool,
    /// R2 rung 3 slice 6: whether THIS place's untracked fields read the
    /// born zero -- true only for self-rooted machine-owned places in the
    /// never-re-entered boot state. Parameter/local-rooted places arrive
    /// with UNKNOWN valuations (poison until a whole-place literal
    /// reseeds).
    born_zero: bool,
    /// R2 rung 3 slice 8 (ch11): an INVARIANT WINDOW -- a checkable write
    /// left the facts FALSE; every consumption point (a read of the place,
    /// a call, state exit) refuses until a later write folds them true.
    window_open: bool,
    /// CASE-CONSTRAINTS (ch12): the place's currently established case, when
    /// a case literal (or a transported valuation) proves it. Case-local
    /// `where` facts fold on payload writes exactly as type-wide facts fold
    /// on common-field writes; `None` means the case is unknown and a
    /// fact-mentioned payload write joins every candidate case's facts.
    active_case: Option<symbols::SymbolHandle>,
}

/// Walk one state (write obligations + the access gate), seeded with the
/// places ESTABLISHED AT ENTRY (the cross-state fixpoint). Returns the
/// EXIT-established spellings (entry-established places stay established:
/// monotone).
fn walk_state(
    program: &TypedTrees,
    call_frames: Option<&crate::machine_calls::calls::CallFrameResolver<'_>>,
    machine: &Machine,
    state: &State,
    entry_established: &[String],
    entry_valuations: &[PlaceValuation],
    entry_windows: &[InvariantWindow],
    summaries: &[(symbols::SymbolHandle, Vec<String>)],
    born_zero: bool,
    exit_is_terminal: bool,
    diagnostics: &mut Vec<Diagnostic>,
    crash_sites: &mut Vec<OpenInvariantCrashSite>,
    record_crash_sites: bool,
) -> (Vec<String>, Vec<PlaceValuation>, Vec<InvariantWindow>) {
    let mut tracked: Vec<TrackedPlace> = Vec::new();
    // Known calls poison only transported valuations they may write. An
    // opaque call poisons every valuation; establishment survives either
    // case because it is globally monotone.
    let mut poisoned_all = false;
    let mut poisoned_paths: Vec<String> = Vec::new();
    // Slice 11: establishment ADDED by callee summaries at call sites.
    let mut call_established: Vec<String> = Vec::new();
    // WINDOW TRANSPORT: windows still open from predecessor states
    // ((spelling, data name) pairs, MAY-union over predecessors). A write
    // in this state that re-proves the facts closes the inherited window;
    // calls and ordinary TERMINAL exits stay hard consumption points; an
    // explicit crash may abandon the window only by retaining damage evidence.
    let mut inherited_windows: Vec<InvariantWindow> = entry_windows.to_vec();
    let mut has_explicit_crash = false;

    for (statement_ordinal, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        // R2 rung 3 slice 2: reads of an unestablished GATED place refuse
        // BEFORE this statement's own write effect is applied.
        scan_statement_reads(
            program,
            machine,
            state,
            statement,
            &tracked,
            entry_established,
            &call_established,
            &inherited_windows,
            diagnostics,
        );
        match statement {
            StatementNode::Assignment(assignment) => {
                // Calls hidden in the written value or in the target's index
                // expressions are still consumption points: they observe the
                // pre-write state and may poison every tracked valuation.
                let index_expressions = write_target_index_expressions(program, assignment.target);
                if expression_contains_call(program, assignment.value)
                    || index_expressions
                        .iter()
                        .any(|index| expression_contains_call(program, *index))
                {
                    refuse_open_windows(&tracked, &inherited_windows, "a call", diagnostics);
                    preserve_proven_establishment(&tracked, &mut call_established);
                    tracked.clear();
                    poisoned_all = true;
                    collect_call_summaries(
                        program,
                        assignment.value,
                        summaries,
                        &mut call_established,
                    );
                    for index in &index_expressions {
                        collect_call_summaries(program, *index, summaries, &mut call_established);
                    }
                }
                handle_assignment(
                    program,
                    machine,
                    state,
                    assignment.target,
                    assignment.value,
                    &mut tracked,
                    entry_valuations,
                    poisoned_all,
                    &poisoned_paths,
                    born_zero,
                    diagnostics,
                );
                // A write that re-proved the facts (tracked window CLOSED)
                // closes the inherited window on the same place.
                inherited_windows.retain(|(spelling, _, _)| {
                    !tracked
                        .iter()
                        .any(|place| place.spelling == *spelling && !place.window_open)
                });
            }
            // A call OBSERVES state, so it remains a hard consumption point
            // (ch11): every open window must have closed. After that check,
            // R5 summaries preserve exact valuations outside known writes.
            StatementNode::Call(call) => {
                refuse_open_windows(&tracked, &inherited_windows, "a call", diagnostics);
                preserve_proven_establishment(&tracked, &mut call_established);
                let written = call_frames.and_then(|frames| frames.may_write_paths(machine, call));
                if let Some(written) = written {
                    tracked.retain(|place| {
                        !written.iter().any(|written| {
                            crate::machine_calls::calls::frame_paths_overlap(
                                &place.spelling,
                                written,
                            )
                        })
                    });
                    for written in written {
                        if !poisoned_paths.contains(&written) {
                            poisoned_paths.push(written);
                        }
                    }
                } else {
                    tracked.clear();
                    poisoned_all = true;
                }
                // Slice 11: the callee's establishment summary joins
                // (call.target_symbol is the target STATE's symbol; resolve
                // to its owning machine).
                let target_machine = machine_symbol_for_state(program, call.target_symbol);
                if let Some((_, established)) = summaries
                    .iter()
                    .find(|(symbol, _)| *symbol == target_machine)
                {
                    call_established.extend(established.iter().cloned());
                }
            }
            StatementNode::Expression(expression) => {
                if expression_contains_call(program, *expression) {
                    refuse_open_windows(&tracked, &inherited_windows, "a call", diagnostics);
                    preserve_proven_establishment(&tracked, &mut call_established);
                    tracked.clear();
                    poisoned_all = true;
                    collect_call_summaries(program, *expression, summaries, &mut call_established);
                }
            }
            StatementNode::LocalData(local) => {
                if local.initial_value.is_valid()
                    && expression_contains_call(program, local.initial_value)
                {
                    refuse_open_windows(&tracked, &inherited_windows, "a call", diagnostics);
                    preserve_proven_establishment(&tracked, &mut call_established);
                    tracked.clear();
                    poisoned_all = true;
                    collect_call_summaries(
                        program,
                        local.initial_value,
                        summaries,
                        &mut call_established,
                    );
                }
            }
            StatementNode::Transition(transition) => {
                if matches!(
                    transition.exit,
                    typed_trees::statement::TransitionExit::Crash(_)
                ) {
                    has_explicit_crash = true;
                    if record_crash_sites {
                        let mut open_data = tracked
                            .iter()
                            .filter(|place| place.window_open)
                            .map(|place| place.definition.symbol)
                            .chain(
                                inherited_windows
                                    .iter()
                                    .map(|(_, _, data_symbol)| *data_symbol),
                            )
                            .collect::<Vec<_>>();
                        open_data.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
                        open_data.dedup();
                        if !open_data.is_empty() {
                            crash_sites.push(OpenInvariantCrashSite {
                                machine: machine.symbol,
                                state: state.symbol,
                                statement_ordinal: u32::try_from(statement_ordinal).expect(
                                    "state-local statement ordinal exceeds crash evidence range",
                                ),
                                open_data,
                            });
                        }
                    }
                }
                // A transition's evaluated expressions (guard, named-target
                // arguments, value target) may carry calls: those are hard
                // consumption points and opaque writes exactly as a bare call
                // statement is. The crash-site evidence above was collected
                // first so this poisoning cannot erase it.
                let evaluated = transition_evaluated_expressions(program, transition);
                if evaluated
                    .iter()
                    .any(|expression| expression_contains_call(program, *expression))
                {
                    refuse_open_windows(&tracked, &inherited_windows, "a call", diagnostics);
                    preserve_proven_establishment(&tracked, &mut call_established);
                    tracked.clear();
                    poisoned_all = true;
                    for expression in &evaluated {
                        collect_call_summaries(
                            program,
                            *expression,
                            summaries,
                            &mut call_established,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    // Ch11 (slice 8, transport-relaxed): a TERMINAL exit is a consumption
    // point -- an open window may not escape the machine. A non-terminal
    // exit passes its open windows to the successors (the fixpoint
    // MAY-unions them), whose own consumption points police closure.
    if exit_is_terminal && !has_explicit_crash {
        refuse_open_windows(&tracked, &inherited_windows, "state exit", diagnostics);
    }

    let mut exit_established: Vec<String> = entry_established.to_vec();
    exit_established.extend(call_established.iter().cloned());
    exit_established.extend(
        tracked
            .iter()
            // Slice 6: parameters are per-invocation -- only machine-owned
            // places transport across states.
            .filter(|place| place.established && is_self_rooted(&place.spelling))
            .map(|place| place.spelling.clone()),
    );
    if attached_value_established(
        program,
        machine,
        &tracked,
        entry_established,
        &call_established,
    ) {
        exit_established.push("self".to_owned());
    }
    exit_established.sort();
    exit_established.dedup();

    // Exit valuations: in-state tracked places, plus entry places that no
    // known write overlaps. An opaque call poisons every untouched entry.
    let mut exit_valuations: Vec<PlaceValuation> = tracked
        .iter()
        .filter(|place| is_self_rooted(&place.spelling))
        .map(|place| {
            (
                place.spelling.clone(),
                place.fields.clone(),
                place.active_case,
            )
        })
        .collect();
    if !poisoned_all {
        for (spelling, fields, active_case) in entry_valuations {
            let poisoned = poisoned_paths
                .iter()
                .any(|written| crate::machine_calls::calls::frame_paths_overlap(spelling, written));
            if !poisoned && !exit_valuations.iter().any(|(name, _, _)| name == spelling) {
                exit_valuations.push((spelling.clone(), fields.clone(), *active_case));
            }
        }
    }
    // Exit windows: inherited ones not closed here, plus windows this
    // state's own writes opened (self-rooted only -- parameters are
    // per-invocation).
    let mut exit_windows = inherited_windows;
    for place in tracked
        .iter()
        .filter(|place| place.window_open && is_self_rooted(&place.spelling))
    {
        if !exit_windows
            .iter()
            .any(|(spelling, _, _)| *spelling == place.spelling)
        {
            exit_windows.push((
                place.spelling.clone(),
                place.definition.name.as_str().to_owned(),
                place.definition.symbol,
            ));
        }
    }
    (exit_established, exit_valuations, exit_windows)
}

//! R2 rung 3 slice 1 (ch12 "Dependent Data"): the default-domain WRITE
//! obligation -- every store to a `where`-mentioned field of a
//! domain-carrying place must leave the facts TRUE at the post-write
//! valuation. This is the strict pre-window semantics (ch11's
//! consumption-point windows are the sanctioned ADDITIVE relaxation);
//! obligations land BEFORE hypotheses on purpose -- over-refusal is safe,
//! over-assumption is not, so readers may not assume the facts until the
//! obligation net is total.
//!
//! V1 tracking model: per-state linear walk over `self`-rooted places
//! (machine-owned data is BORN ZEROED -- ch12's machine-owned rule -- so
//! untracked fields read 0). An integer-literal store tracks its value; a
//! runtime-valued store to a where-mentioned field refuses (the entailment
//! integration relaxes this later); a whole-place struct-literal store
//! reseeds the valuation from the literal (already proven at construction
//! by rung 2b). Resolved calls invalidate valuations overlapping their R5
//! may-write paths; opaque calls retain the conservative whole-state fence.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::data::DataDefinition;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

mod call_summaries;
mod place_queries;
mod state_flow;
mod symbolic_values;
mod where_fact_intervals;

use call_summaries::{collect_call_summaries, machine_symbol_for_state};
use place_queries::{
    data_definition_for_expression, domain_definition_by_name, field_is_where_mentioned,
    is_self_rooted, membership_field_name, self_place_spelling,
};
use state_flow::{PlaceValuation, meet_valuations, state_edges};
use symbolic_values::{
    SymbolicValue, expression_contains_call, expression_sequence_measures, expression_symbol,
    expression_symbolic_value, fold_with_valuation, integer_literal_value,
};
pub(crate) use where_fact_intervals::where_fact_interval;

type InvariantWindow = (String, String, symbols::SymbolHandle);

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
    let call_frames = crate::calls::CallFrameResolver::new(program);

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
            let exits: Vec<(Vec<String>, Vec<PlaceValuation>, Vec<InvariantWindow>)> = states
                .iter()
                .enumerate()
                .map(|(index, state)| {
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
                        &mut throwaway,
                        &mut throwaway_crash_sites,
                        false,
                    )
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
                    let exit = &exits[*predecessor].0;
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
                    for window in &exits[*predecessor].2 {
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
                    .filter(|predecessor| entry_valuations[*predecessor].is_some())
                    .collect();
                if visited.is_empty() {
                    continue;
                }
                let mut valuation_meet: Option<Vec<PlaceValuation>> = None;
                for predecessor in visited {
                    let exit = &exits[predecessor].1;
                    valuation_meet = Some(match valuation_meet {
                        None => exit.clone(),
                        Some(current) => meet_valuations(&current, exit),
                    });
                }
                let valuation_meet = valuation_meet.unwrap_or_default();
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
}

/// Walk one state (write obligations + the access gate), seeded with the
/// places ESTABLISHED AT ENTRY (the cross-state fixpoint). Returns the
/// EXIT-established spellings (entry-established places stay established:
/// monotone).
fn walk_state(
    program: &TypedTrees,
    call_frames: Option<&crate::calls::CallFrameResolver<'_>>,
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
                            crate::calls::frame_paths_overlap(&place.spelling, written)
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
            StatementNode::Transition(transition)
                if matches!(
                    transition.exit,
                    typed_trees::statement::TransitionExit::Crash(_)
                ) =>
            {
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
        .map(|place| (place.spelling.clone(), place.fields.clone()))
        .collect();
    if !poisoned_all {
        for (spelling, fields) in entry_valuations {
            let poisoned = poisoned_paths
                .iter()
                .any(|written| crate::calls::frame_paths_overlap(spelling, written));
            if !poisoned && !exit_valuations.iter().any(|(name, _)| name == spelling) {
                exit_valuations.push((spelling.clone(), fields.clone()));
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

/// A call invalidates exact field valuations through aliasing, but it cannot
/// invalidate establishment: every accepted write in every checked callee must
/// leave the default domain true. Preserve that monotone fact before clearing
/// the more precise tracked valuation.
fn preserve_proven_establishment(tracked: &[TrackedPlace<'_>], established: &mut Vec<String>) {
    established.extend(
        tracked
            .iter()
            .filter(|place| place.established && is_self_rooted(&place.spelling))
            .map(|place| place.spelling.clone()),
    );
}

/// Ch11 (slice 8): refuse every open invariant window at a consumption
/// point, naming the place and the point -- both this state's own open
/// windows and the ones transported from predecessor states.
fn refuse_open_windows(
    tracked: &[TrackedPlace<'_>],
    inherited_windows: &[InvariantWindow],
    consumption_point: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for place in tracked.iter().filter(|place| place.window_open) {
        diagnostics.push(Diagnostic::error(format!(
            "data `{}`'s default domain is still FALSE at {consumption_point}: the \
             invariant window opened on `{}` must close first -- restore the \
             `where` facts before this consumption point (ch11)",
            place.definition.name.as_str(),
            place.spelling
        )));
    }
    for (spelling, data_name, _) in inherited_windows {
        if tracked.iter().any(|place| place.spelling == *spelling) {
            // The tracked entry already reported (open) or closed it.
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "data `{data_name}`'s default domain is still FALSE at {consumption_point}: \
             the invariant window opened on `{spelling}` in a predecessor state must \
             close first -- restore the `where` facts before this consumption point \
             (ch11 window transport)"
        )));
    }
}

fn handle_assignment<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
    value: ExpressionHandle,
    tracked: &mut Vec<TrackedPlace<'program>>,
    entry_valuations: &[PlaceValuation],
    poisoned_all: bool,
    poisoned_paths: &[String],
    born_zero: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A whole-place store of a struct literal reseeds the valuation (the
    // literal itself was proven at construction, rung 2b).
    if let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(value)
        && let Some(spelling) = self_place_spelling(program, target)
        && let Some(definition) = domain_definition_by_name(program, literal.type_name.as_str())
    {
        let fields = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .map(|field| {
                (
                    field.name.as_str().to_string(),
                    integer_literal_value(program, field.value),
                )
            })
            .collect();
        let symbols = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .filter_map(|field| {
                expression_symbolic_value(program, field.value)
                    .map(|symbol| (field.name.as_str().to_string(), symbol))
            })
            .collect();
        let measures = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .filter_map(|field| {
                expression_sequence_measures(program, field.value)
                    .map(|(length, capacity)| (field.name.as_str().to_string(), length, capacity))
            })
            .collect();
        tracked.retain(|place| place.spelling != spelling);
        let place_born_zero = born_zero && is_self_rooted(&spelling);
        tracked.push(TrackedPlace {
            spelling,
            definition,
            fields,
            symbols,
            measures,
            // Rung 2b proved this literal against the domain.
            established: true,
            born_zero: place_born_zero,
            window_open: false,
        });
        return;
    }

    // A FIELD store: `<self-place>.field = value` where the receiver's type
    // carries where facts.
    let ExpressionNode::Member(member) = program.expression_table.expression(target) else {
        return;
    };
    let Some(receiver_spelling) = self_place_spelling(program, member.receiver) else {
        return;
    };
    let Some(definition) =
        data_definition_for_expression(program, machine, Some(state), member.receiver)
    else {
        return;
    };
    if definition.where_facts.is_empty()
        && !crate::data::data_requires_establishment(program, definition)
    {
        return;
    }
    let field_name = member.member.as_str().to_string();
    let written = integer_literal_value(program, value);

    let place = if let Some(position) = tracked
        .iter()
        .position(|place| place.spelling == receiver_spelling)
    {
        &mut tracked[position]
    } else {
        // R2 rung 3 slice 5: seed the fresh place from its transported entry
        // valuation unless an opaque call, or a known overlapping write,
        // poisoned this place's view.
        let poisoned = poisoned_all
            || poisoned_paths
                .iter()
                .any(|written| crate::calls::frame_paths_overlap(&receiver_spelling, written));
        let seeded_fields = if poisoned {
            Vec::new()
        } else {
            entry_valuations
                .iter()
                .find(|(name, _)| *name == receiver_spelling)
                .map(|(_, fields)| fields.clone())
                .unwrap_or_default()
        };
        let self_rooted = is_self_rooted(&receiver_spelling);
        tracked.push(TrackedPlace {
            spelling: receiver_spelling,
            definition,
            fields: seeded_fields,
            symbols: Vec::new(),
            measures: Vec::new(),
            // Zero-satisfying data is born established; gated data must
            // earn it (the accepted write below does, since it re-proves
            // the whole domain). A parameter place arrives ALREADY VALID
            // (the caller's net enforced its domain), so it counts as
            // established for the access gate; its VALUATION stays unknown.
            established: !crate::data::data_requires_establishment(program, definition)
                || !self_rooted,
            born_zero: born_zero && self_rooted,
            window_open: false,
        });
        let last = tracked.len() - 1;
        &mut tracked[last]
    };
    place.fields.retain(|(name, _)| *name != field_name);
    place.fields.push((field_name.clone(), written));
    place.symbols.retain(|(name, _)| *name != field_name);
    if let Some(symbol) = expression_symbolic_value(program, value) {
        place.symbols.push((field_name.clone(), symbol));
    }
    place.measures.retain(|(name, _, _)| *name != field_name);
    if let Some((length, capacity)) = expression_sequence_measures(program, value) {
        place.measures.push((field_name.clone(), length, capacity));
    }

    // Obligation: a field participating in either an authored `where` fact or
    // an implicit range/containment gate must help re-establish the whole
    // value. Unrelated writes preserve the current establishment state.
    let field_type =
        program
            .data_members(place.definition)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == field_name =>
                {
                    Some(field.type_reference)
                }
                _ => None,
            });
    if !field_is_where_mentioned(program, place.definition, &field_name)
        && !field_type
            .is_some_and(|field_type| crate::data::type_requires_establishment(program, field_type))
    {
        return;
    }
    let valuation: Vec<(&str, Option<i128>)> = place
        .fields
        .iter()
        .map(|(name, value)| (name.as_str(), *value))
        .collect();
    let mut all_hold = range_gates_hold(program, place);
    for fact in program
        .proof_facts
        .span_or_empty(place.definition.where_facts)
    {
        match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                match fold_with_valuation(
                    program,
                    &valuation,
                    &place.symbols,
                    &place.measures,
                    place.born_zero,
                    *expression,
                ) {
                    Some(value) if value != 0 => {}
                    // Ch11 (slice 8): a checkable violation OPENS a window instead
                    // of refusing -- the consumption points demand closure.
                    Some(_) => all_hold = false,
                    None => {
                        all_hold = false;
                        // A named runtime value may be written into multiple
                        // correlated fields inside one invariant window. Its
                        // stable symbol lets a later write prove equality; do
                        // not reject before that closing write arrives.
                        if expression_symbol(program, value).is_none() {
                            diagnostics.push(Diagnostic::error(format!(
                                "write to `{}.{field_name}` cannot PROVE data `{}`'s default domain: \
                                 a `where`-mentioned field's value is not a literal known here (a \
                                 runtime value, or a co-field last written in another state) -- \
                                 restructure with literal stores in one state for now (the \
                                 entailment integration and cross-state valuation transport relax \
                                 this)",
                                place.spelling,
                                place.definition.name.as_str()
                            )));
                        }
                    }
                }
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                let mentioned = membership_field_name(program, membership.value);
                if mentioned == Some(field_name.as_str()) {
                    if !crate::proof_facts::string_literal_grants_domain(
                        program,
                        value,
                        membership.domain_symbol,
                    ) {
                        all_hold = false;
                    }
                } else if !place.established || place.window_open {
                    // A write to another field preserves a previously true
                    // membership, but cannot manufacture a missing one.
                    all_hold = false;
                }
            }
            typed_trees::domain::ProofFact::Proposition(_) => {
                // Proposition entailment is proof-layer work. The default-domain
                // interval checker must not pretend a proposition is Boolean.
                all_hold = false;
            }
        }
    }
    // Every fact re-proven at the post-write valuation: the place
    // satisfies its domain again (any open window CLOSES; a gated place
    // establishes). A checkable violation leaves the window OPEN for the
    // consumption points to police (ch11).
    if all_hold {
        place.established = true;
        place.window_open = false;
    } else {
        place.window_open = true;
    }
}

/// R2 rung 3 slice 2: refuse reads of an unestablished GATED place. V1
/// scans value-position expressions for member chains whose self-rooted
/// receiver names a tracked-or-fresh gated place; cross-state
/// establishment is not trackable yet and refuses with direction.
fn scan_statement_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
    inherited_windows: &[InvariantWindow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut reads: Vec<ExpressionHandle> = Vec::new();
    match statement {
        // An assembly fact is itself a checked consumption point. Its proof
        // checker handles establishment; this runtime-read scan must not
        // interpret it as an executed expression.
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => reads.push(assignment.value),
        StatementNode::Expression(expression) => reads.push(*expression),
        StatementNode::LocalData(local) => {
            if local.initial_value.is_valid() {
                reads.push(local.initial_value);
            }
        }
        StatementNode::Call(call) => {
            let receiver = program.statement_table.name_path_members(call.receiver);
            if receiver.len() > 1
                && receiver[0].as_str() == "self"
                && let Some(definition) = machine.attached_data.as_ref().and_then(|attached| {
                    program
                        .data_definitions()
                        .iter()
                        .find(|definition| definition.name == *attached)
                })
                && crate::data::data_requires_establishment(program, definition)
            {
                validate_data_read(
                    program,
                    machine,
                    definition,
                    "self",
                    receiver[1].as_str(),
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
            reads.extend(
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied(),
            );
        }
        StatementNode::Transition(transition) => {
            if let typed_trees::statement::TransitionGuardNode::When(guard) = &transition.guard {
                reads.push(*guard);
            }
        }
    }
    for read in reads {
        scan_expression_reads(
            program,
            machine,
            state,
            read,
            tracked,
            entry_established,
            call_established,
            inherited_windows,
            diagnostics,
        );
    }
}

fn scan_expression_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
    inherited_windows: &[InvariantWindow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members.len() == 1
                && members[0].as_str() == "self"
                && let Some(definition) = machine.attached_data.as_ref().and_then(|attached| {
                    program
                        .data_definitions()
                        .iter()
                        .find(|definition| definition.name == *attached)
                })
                && let Some(place) = tracked.iter().find(|place| place.spelling == "self")
                && place.window_open
            {
                diagnostics.push(Diagnostic::error(format!(
                    "the next whole-value read of `self` occurs inside an OPEN invariant \
                     window: a prior write left data `{}`'s default domain FALSE -- \
                     restore the facts before copying or exposing the value (ch11)",
                    definition.name.as_str()
                )));
            }
            if members.len() > 1 {
                let receiver_spelling = members[..members.len() - 1]
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                if let Some(place) = tracked
                    .iter()
                    .find(|place| place.spelling == receiver_spelling)
                {
                    validate_data_read(
                        program,
                        machine,
                        place.definition,
                        &receiver_spelling,
                        members.last().expect("non-empty path").as_str(),
                        tracked,
                        entry_established,
                        call_established,
                        inherited_windows,
                        diagnostics,
                    );
                }
            }
            // A machine call such as `self.console.exit_process(..)` stores
            // `self.console` as the call receiver Name path. It still consumes
            // the attached `self` value, so it must not bypass establishment
            // merely because no standalone Member node was built.
            if members.len() > 1
                && members[0].as_str() == "self"
                && let Some(definition) = machine.attached_data.as_ref().and_then(|attached| {
                    program
                        .data_definitions()
                        .iter()
                        .find(|definition| definition.name == *attached)
                })
            {
                validate_data_read(
                    program,
                    machine,
                    definition,
                    "self",
                    members[1].as_str(),
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Member(member) => {
            if let Some(receiver_spelling) = self_place_spelling(program, member.receiver) {
                let tracked_receiver = tracked
                    .iter()
                    .find(|place| place.spelling == receiver_spelling);
                let definition = tracked_receiver.map(|place| place.definition).or_else(|| {
                    data_definition_for_expression(program, machine, Some(state), member.receiver)
                });
                if let Some(definition) = definition
                    && crate::data::data_requires_establishment(program, definition)
                {
                    validate_data_read(
                        program,
                        machine,
                        definition,
                        &receiver_spelling,
                        member.member.as_str(),
                        tracked,
                        entry_established,
                        call_established,
                        inherited_windows,
                        diagnostics,
                    );
                }
            }
            if !is_bare_self_name(program, member.receiver) {
                scan_expression_reads(
                    program,
                    machine,
                    state,
                    member.receiver,
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Indexed(indexed) => {
            scan_expression_reads(
                program,
                machine,
                state,
                indexed.collection,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
            scan_expression_reads(
                program,
                machine,
                state,
                indexed.index,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
        }
        ExpressionNode::Binary(binary) => {
            scan_expression_reads(
                program,
                machine,
                state,
                binary.left,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
            scan_expression_reads(
                program,
                machine,
                state,
                binary.right,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
        }
        ExpressionNode::Borrow(inner) => {
            if let ExpressionNode::Member(member) =
                program.expression_table.expression(inner.target)
                && let Some(receiver_spelling) = self_place_spelling(program, member.receiver)
                && let Some(place) = tracked
                    .iter()
                    .find(|place| place.spelling == receiver_spelling && place.window_open)
            {
                validate_data_read(
                    program,
                    machine,
                    place.definition,
                    &receiver_spelling,
                    member.member.as_str(),
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
            scan_expression_reads(
                program,
                machine,
                state,
                inner.target,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
        }
        ExpressionNode::Call(call) => {
            scan_expression_reads(
                program,
                machine,
                state,
                call.receiver,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                diagnostics,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_expression_reads(
                    program,
                    machine,
                    state,
                    *argument,
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

fn is_bare_self_name(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    let members = program.expression_table.name_path_members(path.members);
    members.len() == 1 && members[0].as_str() == "self"
}

fn validate_data_read(
    program: &TypedTrees,
    machine: &Machine,
    definition: &DataDefinition,
    receiver_spelling: &str,
    member_name: &str,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
    inherited_windows: &[InvariantWindow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let place = tracked
        .iter()
        .find(|place| place.spelling == receiver_spelling);
    let established = place.map(|place| place.established).unwrap_or_else(|| {
        // Parameters and locals arrive domain-valid from their caller or
        // initializer; machine-owned `self` storage starts as representation
        // only and must earn establishment on every incoming path.
        entry_established
            .iter()
            .any(|established| established == receiver_spelling)
            || call_established
                .iter()
                .any(|established| established == receiver_spelling)
            || !is_self_rooted(receiver_spelling)
    });
    let established = established
        || (receiver_spelling == "self"
            && attached_value_established(
                program,
                machine,
                tracked,
                entry_established,
                call_established,
            ));
    if !established {
        diagnostics.push(Diagnostic::error(format!(
            "reading `{receiver_spelling}.{member_name}` crosses an open default-domain \
             invariant window before data `{}` is established: the zeroed representation \
             is not yet a `{}` (ch12's access gate) -- construct it on every path first \
             (the cross-state must-analysis carries establishment)",
            definition.name.as_str(),
            definition.name.as_str()
        )));
    }
    if place.is_some_and(|place| place.window_open) {
        diagnostics.push(Diagnostic::error(format!(
            "reading `{receiver_spelling}.{member_name}` inside an OPEN invariant window: \
             a prior write left data `{}`'s default domain FALSE -- restore the facts \
             before this consumption point (ch11)",
            definition.name.as_str()
        )));
    }
    if place.is_none()
        && inherited_windows
            .iter()
            .any(|(spelling, _, _)| spelling == receiver_spelling)
    {
        diagnostics.push(Diagnostic::error(format!(
            "reading `{receiver_spelling}.{member_name}` inside an OPEN invariant window \
             carried from a predecessor state: data `{}`'s default domain is FALSE -- \
             restore the facts before this consumption point (ch11 window transport)",
            definition.name.as_str()
        )));
    }
}

/// A nested gated field gates its containing machine value, but establishing
/// that child must in turn establish the parent once every gated child is
/// ready. This is the establishment analogue of structural ZII composition:
/// the parent carries no independent ceremony when its own authored default
/// domain accepts zero.
fn attached_value_established(
    program: &TypedTrees,
    machine: &Machine,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
) -> bool {
    if direct_place_established("self", tracked, entry_established, call_established) {
        return true;
    }
    let Some(attached) = machine.attached_data.as_ref() else {
        return false;
    };
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name == *attached)
    else {
        return false;
    };
    // An authored default domain that rejects zero must be established by its
    // own proof net; child establishment cannot manufacture that evidence.
    if definition.zero_gated {
        return false;
    }
    let root = tracked.iter().find(|place| place.spelling == "self");
    for member in program.data_members(definition) {
        let typed_trees::data::DataMember::Field(field) = member else {
            continue;
        };
        if !crate::data::type_requires_establishment(program, field.type_reference) {
            continue;
        }
        if let Some(interval) =
            crate::arithmetic_domains::range_constraint_interval(program, field.type_reference)
        {
            let Some(value) = root
                .and_then(|place| {
                    place
                        .fields
                        .iter()
                        .find(|(name, _)| name == field.name.as_str())
                })
                .and_then(|(_, value)| *value)
            else {
                return false;
            };
            if interval.low().is_some_and(|low| value < i128::from(low))
                || interval.high().is_some_and(|high| value > i128::from(high))
            {
                return false;
            }
        } else {
            let child = format!("self.{}", field.name.as_str());
            if !direct_place_established(&child, tracked, entry_established, call_established) {
                return false;
            }
        }
    }
    true
}

fn direct_place_established(
    spelling: &str,
    tracked: &[TrackedPlace<'_>],
    entry_established: &[String],
    call_established: &[String],
) -> bool {
    tracked
        .iter()
        .any(|place| place.spelling == spelling && place.established)
        || entry_established.iter().any(|place| place == spelling)
        || call_established.iter().any(|place| place == spelling)
}

/// Implicit range/containment gates hold only after every zero-excluding
/// common field has a known established value. Scalar ranges can be discharged
/// by the ordinary literal valuation. Nested data and arrays are established
/// by whole-value construction for now; a scalar field write cannot fabricate
/// evidence for them.
fn range_gates_hold(program: &TypedTrees, place: &TrackedPlace<'_>) -> bool {
    for member in program.data_members(place.definition) {
        let typed_trees::data::DataMember::Field(field) = member else {
            continue;
        };
        if !crate::data::type_requires_establishment(program, field.type_reference) {
            continue;
        }
        let Some(interval) =
            crate::arithmetic_domains::range_constraint_interval(program, field.type_reference)
        else {
            // Nested records and arrays require whole-value establishment in
            // this first slice.
            return false;
        };
        let Some(value) = place
            .fields
            .iter()
            .find(|(name, _)| name == field.name.as_str())
            .and_then(|(_, value)| *value)
        else {
            return false;
        };
        if interval.low().is_some_and(|low| value < i128::from(low))
            || interval.high().is_some_and(|high| value > i128::from(high))
        {
            return false;
        }
    }
    true
}

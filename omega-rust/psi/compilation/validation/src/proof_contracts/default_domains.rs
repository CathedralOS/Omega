//! Let a helper update a record's fields in several steps, while requiring the
//! record to be valid before a call or return exposes it. A temporary violation
//! opens an invariant window: writes can restore it, but the record's own
//! requirements cannot justify a read while they remain unproved.
//!
//! Track each requirement separately from known field values. Updating one
//! field invalidates overlapping requirements; it does not erase a proof about
//! an untouched sibling. This matters for borrowed records whose callers may
//! supply arbitrary values that satisfy the declared requirements.
//!
//! State walks also retain established storage by intersection, literal values
//! by agreement, and open windows by union. Machine-owned storage begins at
//! zero only on a non-reentrant entry; parameters and initialized locals arrive
//! with established values, whose actual contents may be unknown. Known
//! statement-call write sets preserve unrelated field values. Calls in
//! expressions may conservatively clear those values; checked record validity
//! is carried separately.
//!
//! Reserved storage has zero bits but may still lack a valid value. Establish
//! its children and containing predicates before consuming it. Projecting an
//! address does not consume each parent record, but reading a qualified field
//! must still establish that field's own requirements.
//!
//! This module owns the state walk. `assignment_windows.rs` updates the tracked
//! requirements after stores; `data_reads.rs` checks reads of those places.

mod assignment_windows;
pub(crate) mod call_summaries;
mod case_requirements;
mod data_reads;
mod element_stores;
mod float_predicates;
mod place_queries;
mod post_store_predicates;
mod standing_requirements;
mod state_flow;
mod symbolic_values;
mod where_fact_intervals;

pub use super::contract_entailment::data_predicates_admit_zero;
pub(crate) use case_requirements::selected_case_requirements;
pub use float_predicates::data_float_predicate;
pub(crate) use float_predicates::float_field_literal_bound_meaning;
pub(crate) use place_queries::completed_case_occurrences;
pub(crate) use place_queries::data_predicate_fields;
pub(crate) use place_queries::membership_field;
pub(crate) use place_queries::predicate_projection;
pub use standing_requirements::standing_facts_prove_requirement;
pub(crate) use symbolic_values::applied_const_integer;
pub(crate) use where_fact_intervals::{
    FieldRepresentationBound, atomic_field_interval, data_where_field_representation_bounds,
    stored_field_where_interval,
};
pub use where_fact_intervals::{
    case_where_field_intervals, data_where_field_intervals, impossible_variant,
};

use diagnostics::Diagnostic;
use typed_trees::data::DataDefinition;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

use crate::proof_contracts::default_domains::assignment_windows::handle_assignment;
use crate::proof_contracts::default_domains::assignment_windows::preserve_proven_establishment;
use crate::proof_contracts::default_domains::assignment_windows::refuse_open_windows;
use crate::proof_contracts::default_domains::data_reads::attached_value_established;
use crate::proof_contracts::default_domains::data_reads::scan_statement_reads;
use call_summaries::{StateToMachine, collect_call_summaries, state_to_machine_index};
use place_queries::{
    is_data_rooted, transition_evaluated_expressions, write_target_index_expressions,
};
use state_flow::{PlaceValuation, canonicalize_valuations, meet_valuations, state_edges};
use symbolic_values::{SymbolicValue, expression_contains_call};

type InvariantWindow = (String, String, symbols::SymbolHandle);

/// Parallel walks share results only within this validation invocation. A
/// different compilation must not clear or contribute to these decisions.
type ParameterCallAttempts = std::sync::Mutex<Vec<(symbols::SymbolHandle, bool)>>;

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

/// One state's walk under its current entry: its exit and what that walk
/// reports.
struct StateWalk {
    exit: StateExit,
    diagnostics: Vec<Diagnostic>,
    crash_sites: Vec<OpenInvariantCrashSite>,
}

/// Each state's predecessors, in edge order.
fn state_predecessors(edges: &[(usize, usize)], state_count: usize) -> Vec<Vec<usize>> {
    let mut predecessors = vec![Vec::new(); state_count];
    for (from, to) in edges {
        predecessors[*to].push(*from);
    }
    predecessors
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
    program: &dyn crate::CheckingInput,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    analyze_default_domain_writes_with_frames(program, call_frames, diagnostics, &mut Vec::new());
}

fn analyze_default_domain_writes_with_frames(
    program: &dyn crate::CheckingInput,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
    crash_sites: &mut Vec<OpenInvariantCrashSite>,
) {
    let parameter_call_attempts = ParameterCallAttempts::default();
    // Every call site resolves its target machine through this index; the
    // program is frozen for the pass so one build covers all lookups.
    let state_to_machine = state_to_machine_index(program);
    // Summary-construction walks join no nested summaries (conservative);
    // the empty map is shared so it is not rebuilt per walk.
    let no_summaries = symbols::SymbolKeyMap::default();

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
    let mut summaries: symbols::SymbolKeyMap<symbols::SymbolHandle, Vec<String>> =
        symbols::SymbolKeyMap::default();
    let machine_summaries: Vec<(symbols::SymbolHandle, Vec<String>)> =
        crate::checking_thread_pool().install(|| {
            use rayon::prelude::*;
            program
                .machines()
                .par_iter()
                .filter_map(|machine| {
                    let states = program.machine_states(machine);
                    if states.is_empty() {
                        return None;
                    }
                    // A callee's per-invocation parameters do not outlive it; only the
                    // places rooted at its own data binding are what a caller may rely on.
                    let callee_binding = program
                        .machine_data_binding(machine)
                        .map(|name| name.as_str())?;
                    let mut throwaway = Vec::new();
                    let mut throwaway_crash_sites = Vec::new();
                    let (exits, terminal): (Vec<Vec<String>>, Vec<usize>) = if states.len() == 1 {
                        (
                            vec![
                                walk_state(
                                    program,
                                    call_frames,
                                    &parameter_call_attempts,
                                    machine,
                                    &states[0],
                                    &[],
                                    &[],
                                    &[],
                                    &no_summaries,
                                    &state_to_machine,
                                    false,
                                    true,
                                    &mut throwaway,
                                    &mut throwaway_crash_sites,
                                    false,
                                    &[],
                                )
                                .0,
                            ],
                            vec![0],
                        )
                    } else {
                        let edges = state_edges(program, states);
                        let predecessors = state_predecessors(&edges, states.len());
                        let mut entry: Vec<Vec<String>> = vec![Vec::new(); states.len()];
                        // A walk reads only its own entry, so a state is walked again only
                        // after its entry changed.
                        let mut exits: Vec<Vec<String>> = vec![Vec::new(); states.len()];
                        let mut stale = vec![true; states.len()];
                        loop {
                            for (index, state) in states.iter().enumerate() {
                                if !std::mem::take(&mut stale[index]) {
                                    continue;
                                }
                                exits[index] = walk_state(
                                    program,
                                    call_frames,
                                    &parameter_call_attempts,
                                    machine,
                                    state,
                                    &entry[index],
                                    &[],
                                    &[],
                                    &no_summaries,
                                    &state_to_machine,
                                    false,
                                    true,
                                    &mut throwaway,
                                    &mut throwaway_crash_sites,
                                    false,
                                    &[],
                                )
                                .0;
                            }
                            let mut changed = false;
                            for (index, current_entry) in entry.iter_mut().enumerate().skip(1) {
                                let predecessors = &predecessors[index];
                                if predecessors.is_empty() {
                                    continue;
                                }
                                let mut meet: Option<Vec<String>> = None;
                                for predecessor in predecessors {
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
                                    stale[index] = true;
                                    changed = true;
                                }
                            }
                            if !changed {
                                break;
                            }
                        }
                        let terminal: Vec<usize> = (0..states.len())
                            .filter(|index| !edges.iter().any(|(from, _)| from == index))
                            .collect();
                        (exits, terminal)
                    };
                    if terminal.is_empty() {
                        return None;
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
                    let data_rooted: Vec<String> = definite
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|spelling| is_data_rooted(Some(callee_binding), spelling))
                        .collect();
                    (!data_rooted.is_empty()).then_some((machine.symbol, data_rooted))
                })
                .collect()
        });
    for (symbol, data_rooted) in machine_summaries {
        summaries.insert(symbol, data_rooted);
    }

    let machine_results: Vec<(
        symbols::SymbolHandle,
        Vec<Diagnostic>,
        Vec<OpenInvariantCrashSite>,
    )> = crate::checking_thread_pool().install(|| {
        use rayon::prelude::*;
        program
            .machines()
            .par_iter()
            .filter_map(|machine| {
                let states = program.machine_states(machine);
                // Bodyless machines (boundary/requirement declarations) own no
                // states -- nothing to walk.
                if states.is_empty() {
                    return None;
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
                let mut entry_valuations: Vec<Option<Vec<PlaceValuation>>> =
                    vec![None; states.len()];
                entry_valuations[0] = Some(Vec::new());
                // WINDOW TRANSPORT: open windows at each state's entry -- the
                // MAY-union of predecessor exits (an obligation from ANY path in).
                let mut entry_windows: Vec<Vec<InvariantWindow>> = vec![Vec::new(); states.len()];
                // A TERMINAL state (no outgoing transition) is where the machine
                // returns: its exit is a hard consumption point for open windows.
                let is_terminal = |index: usize| !edges.iter().any(|(from, _)| *from == index);
                let predecessors = state_predecessors(&edges, states.len());
                // A walk reads only its own entry, so a state is walked again only
                // after its entry changed. Each walk keeps its own diagnostics and
                // crash sites; at the fixed point every state's last walk ran on its
                // final entry, so those are the ones reported.
                let mut walks: Vec<Option<StateWalk>> = (0..states.len()).map(|_| None).collect();
                let mut stale = vec![true; states.len()];
                loop {
                    for (index, state) in states.iter().enumerate() {
                        if !std::mem::take(&mut stale[index]) {
                            continue;
                        }
                        let mut state_diagnostics = Vec::new();
                        let mut state_crash_sites = Vec::new();
                        let (established, valuations, windows) = walk_state(
                            program,
                            call_frames,
                            &parameter_call_attempts,
                            machine,
                            state,
                            &entry_established[index],
                            entry_valuations[index].as_deref().unwrap_or(&[]),
                            &entry_windows[index],
                            &summaries,
                            &state_to_machine,
                            born_zero(index),
                            is_terminal(index),
                            &mut state_diagnostics,
                            &mut state_crash_sites,
                            true,
                            &[],
                        );
                        walks[index] = Some(StateWalk {
                            exit: StateExit {
                                established,
                                valuations,
                                windows,
                                was_reached: entry_valuations[index].is_some(),
                            },
                            diagnostics: state_diagnostics,
                            crash_sites: state_crash_sites,
                        });
                    }
                    let exits = walks
                        .iter()
                        .map(|walk| &walk.as_ref().expect("every state is walked").exit)
                        .collect::<Vec<_>>();
                    let mut changed = false;
                    for index in 1..states.len() {
                        let predecessors = &predecessors[index];
                        if predecessors.is_empty() {
                            continue;
                        }
                        // Establishment meet (intersection over ALL predecessors).
                        let mut established_meet: Option<Vec<String>> = None;
                        for predecessor in predecessors {
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
                            stale[index] = true;
                            changed = true;
                        }
                        // Window MAY-union: open from ANY predecessor -> open here.
                        let mut window_union: Vec<InvariantWindow> = Vec::new();
                        for predecessor in predecessors {
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
                            stale[index] = true;
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
                            stale[index] = true;
                            changed = true;
                        }
                    }
                    if !changed {
                        break;
                    }
                }
                let mut machine_diagnostics = Vec::new();
                let mut machine_crash_sites = Vec::new();
                for walk in walks.into_iter().flatten() {
                    machine_diagnostics.extend(walk.diagnostics);
                    machine_crash_sites.extend(walk.crash_sites);
                }
                (!machine_diagnostics.is_empty() || !machine_crash_sites.is_empty()).then_some((
                    machine.symbol,
                    machine_diagnostics,
                    machine_crash_sites,
                ))
            })
            .collect()
    });
    let proofs = parameter_call_attempts
        .into_inner()
        .expect("parameter call proofs");
    for (symbol, machine_diagnostics, machine_crash_sites) in machine_results {
        let attempts = proofs
            .iter()
            .filter(|(callee, _)| *callee == symbol)
            .map(|(_, proved)| *proved)
            .collect::<Vec<_>>();
        if !attempts.is_empty() && attempts.iter().all(|proved| *proved) {
            continue;
        }
        diagnostics.extend(machine_diagnostics);
        crash_sites.extend(machine_crash_sites);
    }
}

/// A declaration requirement applies to the common fields or one selected case.
/// Keep its identity when another requirement opens an invariant window.
#[derive(Clone, PartialEq, Eq)]
struct DomainRequirement {
    case: symbols::SymbolHandle,
    fact: typed_trees::domain::ProofFact,
}

/// One tracked place: its rendered spelling, its data definition, and the
/// per-field valuation (`None` value = written with a non-literal).
struct TrackedPlace<'program> {
    spelling: String,
    definition: &'program DataDefinition,
    fields: Vec<(String, Option<symbolic_values::LiteralValue>)>,
    /// Children established by an incoming valid record or a checked whole
    /// replacement. This proves their own types, never a containing predicate.
    established_fields: Vec<symbols::SymbolHandle>,
    /// Requirements invalidated by performed writes to this storage. Untouched
    /// requirements remain proved even while another field needs restoration.
    pending_requirements: Vec<DomainRequirement>,
    symbols: Vec<(String, SymbolicValue)>,
    measures: Vec<(String, Option<i128>, Option<i128>)>,
    /// Whether this storage has held an established value. Later writes may
    /// still owe restoration; failed initial establishment creates no value.
    established: bool,
    /// Unwritten fields may use their zero representation at initial machine
    /// entry or in untouched reserved local storage. Incoming parameters and
    /// re-entered states supply no such premise.
    born_zero: bool,
    /// A write left requirements unproved. They must be restored before a
    /// read that relies on them, a call or a return exposes the record.
    window_open: bool,
    /// The first write still awaiting restoration, used to locate a refusal.
    opening_write: Option<source::SourceSpan>,
    /// Element knowledge of fixed integer array fields that element stores
    /// in this state wrote, one set per element.
    elements: Vec<(
        symbols::SymbolHandle,
        Vec<crate::proof_contracts::collection_predicates::IntegerSet>,
    )>,
    /// Array fields whose element stores left a fact reading their contents
    /// unproved. Each keeps the place's window open until a later store to
    /// that field re-proves it; field stores cannot close it.
    element_windows: Vec<symbols::SymbolHandle>,
    /// The selected case, when construction or incoming values establish it.
    /// An unknown tag cannot borrow another case's predicate.
    active_case: Option<symbols::SymbolHandle>,
    /// Each arm of a joined initializer. A later field write preserves the
    /// domain only when every arm still proves it.
    alternatives: Vec<Vec<(String, Option<symbolic_values::LiteralValue>)>>,
}

/// A complete checked value establishes each common child's own type. Preserve
/// that fact when tracking the value, even if its child contents are unknown.
/// Fresh reserved storage must instead establish its fields individually.
fn established_common_fields(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
) -> Vec<symbols::SymbolHandle> {
    program
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_field().map(|field| field.symbol))
        .collect()
}

impl TrackedPlace<'_> {
    /// Whether any invariant window is open on this place: one a field write
    /// opened, or one an element store opened.
    fn any_window_open(&self) -> bool {
        self.window_open
            || !self.element_windows.is_empty()
            || (self.established && !self.pending_requirements.is_empty())
    }
}

fn place_snapshots<'program>(
    tracked: &[TrackedPlace<'program>],
) -> Vec<(
    String,
    Vec<(String, Option<symbolic_values::LiteralValue>)>,
    Option<symbols::SymbolHandle>,
    &'program typed_trees::data::DataDefinition,
)> {
    tracked
        .iter()
        .map(|place| {
            (
                place.spelling.clone(),
                place.fields.clone(),
                place.active_case,
                place.definition,
            )
        })
        .collect()
}

fn seed_transported_initializer<'program>(
    program: &'program dyn crate::CheckingInput,
    local: &typed_trees::statement::TableLocalData,
    tracked: &mut Vec<TrackedPlace<'program>>,
) {
    if !local.initial_value.is_valid() {
        return;
    }
    match program.expression_table.expression(local.initial_value) {
        typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
            let holder =
                crate::field_domains::owned_nominal_data_definition(program, local.type_reference);
            for field in program.expression_table.struct_fields(literal.fields) {
                let Some((case, fields, symbols)) = case_snapshot(program, field.value) else {
                    continue;
                };
                let Some(holder) = holder else {
                    continue;
                };
                let Some(declared) = program.data_members(holder).iter().find_map(|member| {
                    member
                        .as_field()
                        .filter(|candidate| candidate.name.as_str() == field.name.as_str())
                }) else {
                    continue;
                };
                let Some(definition) = crate::field_domains::owned_nominal_data_definition(
                    program,
                    declared.type_reference,
                ) else {
                    continue;
                };
                let spelling = format!("{}.{}", local.name.as_str(), field.name.as_str());
                if tracked.iter().any(|place| place.spelling == spelling) {
                    continue;
                }
                tracked.push(TrackedPlace {
                    spelling,
                    definition,
                    fields,
                    established_fields: established_common_fields(program, definition),
                    pending_requirements: Vec::new(),
                    symbols,
                    measures: Vec::new(),
                    established: true,
                    born_zero: false,
                    window_open: false,
                    opening_write: None,
                    elements: Vec::new(),
                    element_windows: Vec::new(),
                    active_case: Some(case),
                    alternatives: Vec::new(),
                });
            }
        }
        typed_trees::expression::ExpressionNode::Match(matched) => {
            let arms = program.expression_table.match_arms(matched.arms);
            let snapshots = arms
                .iter()
                .filter_map(|arm| case_snapshot(program, arm.value))
                .collect::<Vec<_>>();
            if snapshots.len() != arms.len() || snapshots.is_empty() {
                return;
            }
            let case = snapshots[0].0;
            if snapshots.iter().any(|snapshot| snapshot.0 != case) {
                return;
            }
            let Some(definition) =
                crate::field_domains::owned_nominal_data_definition(program, local.type_reference)
            else {
                return;
            };
            if tracked
                .iter()
                .any(|place| place.spelling == local.name.as_str())
            {
                return;
            }
            tracked.push(TrackedPlace {
                spelling: local.name.to_string(),
                definition,
                fields: Vec::new(),
                established_fields: established_common_fields(program, definition),
                pending_requirements: Vec::new(),
                symbols: Vec::new(),
                measures: Vec::new(),
                established: true,
                born_zero: false,
                window_open: false,
                opening_write: None,
                elements: Vec::new(),
                element_windows: Vec::new(),
                active_case: Some(case),
                alternatives: snapshots.into_iter().map(|(_, fields, _)| fields).collect(),
            });
        }
        typed_trees::expression::ExpressionNode::Call(call) => {
            let Some((case, fields, symbols)) =
                machine_return_snapshot(program, call.target_symbol)
            else {
                return;
            };
            let Some(definition) =
                crate::field_domains::owned_nominal_data_definition(program, local.type_reference)
            else {
                return;
            };
            if tracked
                .iter()
                .any(|place| place.spelling == local.name.as_str())
            {
                return;
            }
            tracked.push(TrackedPlace {
                spelling: local.name.to_string(),
                definition,
                fields,
                established_fields: established_common_fields(program, definition),
                pending_requirements: Vec::new(),
                symbols,
                measures: Vec::new(),
                established: true,
                born_zero: false,
                window_open: false,
                opening_write: None,
                elements: Vec::new(),
                element_windows: Vec::new(),
                active_case: Some(case),
                alternatives: Vec::new(),
            });
        }
        _ => {}
    }
}

fn case_snapshot(
    program: &dyn crate::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(
    symbols::SymbolHandle,
    Vec<(String, Option<symbolic_values::LiteralValue>)>,
    Vec<(String, symbolic_values::SymbolicValue)>,
)> {
    let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
    else {
        return None;
    };
    let case = literal.case_symbol.filter(|symbol| symbol.is_valid())?;
    let (fields, symbols) = symbolic_values::struct_literal_snapshot(program, expression);
    Some((case, fields, symbols))
}

fn machine_return_snapshot(
    program: &dyn crate::CheckingInput,
    target: symbols::SymbolHandle,
) -> Option<(
    symbols::SymbolHandle,
    Vec<(String, Option<symbolic_values::LiteralValue>)>,
    Vec<(String, symbolic_values::SymbolicValue)>,
)> {
    let machine = program.machine(target).or_else(|| {
        // A call target may be the entry state rather than the machine.
        program.machines().iter().find(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == target)
        })
    })?;
    let state = program.machine_states(machine).first()?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let expression = match statements {
        [typed_trees::statement::StatementNode::Expression(expression)] => *expression,
        [typed_trees::statement::StatementNode::Transition(transition)] => {
            let typed_trees::statement::TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            *expression
        }
        _ => return None,
    };
    case_snapshot(program, expression)
}

fn visit_calls(
    program: &dyn crate::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    calls: &mut Vec<typed_trees::expression::TableCallExpression>,
) {
    if !program.expression_table.expression_is_valid(expression) {
        return;
    }
    let node = program.expression_table.expression(expression);
    match &node {
        typed_trees::expression::ExpressionNode::Call(call) => calls.push(call.clone()),
        node => crate::queries::expression_traversal::children(program, node, |child| {
            visit_calls(program, child, calls);
        }),
    }
}

fn restore_parameter_write<'program>(
    program: &'program dyn crate::CheckingInput,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
    parameter_call_attempts: &ParameterCallAttempts,
    target: symbols::SymbolHandle,
    arguments: &[typed_trees::expression::ExpressionHandle],
    state_to_machine: &StateToMachine,
    saved_places: &[(
        String,
        Vec<(String, Option<symbolic_values::LiteralValue>)>,
        Option<symbols::SymbolHandle>,
        &'program typed_trees::data::DataDefinition,
    )],
    tracked: &mut Vec<TrackedPlace<'program>>,
    diagnostics: &mut Vec<Diagnostic>,
    active_callees: &[symbols::SymbolHandle],
) {
    let Some(machine) = program.machine(target).or_else(|| {
        state_to_machine
            .get(&target)
            .and_then(|symbol| program.machine(*symbol))
    }) else {
        return;
    };
    // A specialized walk re-enters callees with the caller's literal field
    // valuations. Private machines that hand the same place back and forth
    // (`head -> tail -> head`) would re-walk forever. A callee already on this
    // chain records an unproved attempt: the re-entry is a real arrival this
    // walk did not prove, so the callee's own unspecialized walk keeps every
    // diagnostic instead of being excused by its other proved attempts.
    if active_callees.contains(&machine.symbol) {
        parameter_call_attempts
            .lock()
            .expect("parameter call proofs")
            .push((machine.symbol, false));
        return;
    }
    let Some(state) = program.machine_states(machine).first() else {
        return;
    };
    let parameters = program.state_parameters(state);
    let mut entry_valuations = Vec::new();
    let mut actuals = Vec::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let Some(spelling) = place_queries::write_place_spelling(program, *argument) else {
            continue;
        };
        let Some(place) = saved_places.iter().find(|place| place.0 == spelling) else {
            continue;
        };
        entry_valuations.push((
            parameter.name.as_str().to_string(),
            place.1.clone(),
            place.2,
        ));
        actuals.push((spelling, parameter.name.as_str().to_string()));
    }
    if entry_valuations.is_empty() {
        return;
    }
    // This specialized check can replace the callee's ordinary diagnostics.
    // It must inspect the same performed writes and reads. Missing write
    // analysis is an unproved attempt, never evidence that the body is safe.
    let Some(call_frames) = call_frames else {
        parameter_call_attempts
            .lock()
            .expect("parameter call proofs")
            .push((machine.symbol, false));
        return;
    };
    let mut callee_diagnostics = Vec::new();
    let mut crash_sites = Vec::new();
    let mut chain = active_callees.to_vec();
    chain.push(machine.symbol);
    walk_state(
        program,
        Some(call_frames),
        parameter_call_attempts,
        machine,
        state,
        &[],
        &entry_valuations,
        &[],
        &symbols::SymbolKeyMap::default(),
        state_to_machine,
        false,
        true,
        &mut callee_diagnostics,
        &mut crash_sites,
        false,
        &chain,
    );
    let proved = callee_diagnostics.is_empty();
    parameter_call_attempts
        .lock()
        .expect("parameter call proofs")
        .push((machine.symbol, proved));
    if !proved {
        diagnostics.extend(callee_diagnostics);
        return;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    for statement in statements {
        let typed_trees::statement::StatementNode::Assignment(assignment) = statement else {
            continue;
        };
        let Some(target) = place_queries::write_place_spelling(program, assignment.target) else {
            continue;
        };
        for (actual, parameter) in &actuals {
            let Some(field) = target.strip_prefix(&format!("{parameter}.")) else {
                continue;
            };
            let written = symbolic_values::literal_value(program, assignment.value);
            if let Some(place) = tracked.iter_mut().find(|place| place.spelling == *actual) {
                if let Some(slot) = place.fields.iter_mut().find(|(name, _)| name == field) {
                    slot.1 = written;
                } else {
                    place.fields.push((field.to_string(), written));
                }
                place.established_fields = established_common_fields(program, place.definition);
                place.window_open = false;
                place.opening_write = None;
                place.pending_requirements.clear();
                place.established = true;
            } else if let Some(saved) = saved_places.iter().find(|place| place.0 == *actual) {
                let mut fields = saved.1.clone();
                if let Some(slot) = fields.iter_mut().find(|(name, _)| name == field) {
                    slot.1 = written;
                } else {
                    fields.push((field.to_string(), written));
                }
                tracked.push(TrackedPlace {
                    spelling: actual.clone(),
                    definition: saved.3,
                    fields,
                    established_fields: established_common_fields(program, saved.3),
                    pending_requirements: Vec::new(),
                    symbols: Vec::new(),
                    measures: Vec::new(),
                    established: true,
                    born_zero: false,
                    window_open: false,
                    opening_write: None,
                    elements: Vec::new(),
                    element_windows: Vec::new(),
                    active_case: saved.2,
                    alternatives: Vec::new(),
                });
            }
        }
    }
}

/// Walk one state (write obligations + the access gate), seeded with the
/// places ESTABLISHED AT ENTRY (the cross-state fixpoint). Returns the
/// EXIT-established spellings (entry-established places stay established:
/// monotone).
fn walk_state(
    program: &dyn crate::CheckingInput,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
    parameter_call_attempts: &ParameterCallAttempts,
    machine: &Machine,
    state: &State,
    entry_established: &[String],
    entry_valuations: &[PlaceValuation],
    entry_windows: &[InvariantWindow],
    summaries: &symbols::SymbolKeyMap<symbols::SymbolHandle, Vec<String>>,
    state_to_machine: &StateToMachine,
    born_zero: bool,
    exit_is_terminal: bool,
    diagnostics: &mut Vec<Diagnostic>,
    crash_sites: &mut Vec<OpenInvariantCrashSite>,
    record_crash_sites: bool,
    active_callees: &[symbols::SymbolHandle],
) -> (Vec<String>, Vec<PlaceValuation>, Vec<InvariantWindow>) {
    // The binding that names the record this state's field places are rooted
    // at: `self` under the receiver spelling, an ordinary parameter name under
    // the static one. Resolved once here -- every rootedness question in this
    // walk is about this one name.
    let data_binding = program
        .state_data_binding(machine, state)
        .map(|name| name.as_str().to_owned());
    let data_binding = data_binding.as_deref();
    let mut tracked: Vec<TrackedPlace> = Vec::new();
    // Known calls poison only transported valuations they may write. An
    // opaque call poisons every valuation; establishment survives either
    // case because it is globally monotone.
    let mut poisoned_all = false;
    let mut poisoned_paths: Vec<String> = Vec::new();
    // In-state exact establishment from accepted stores, surviving nominal
    // values and callee summaries. Unlike valuations, validity survives calls;
    // a later overlapping invariant window still fences every observation.
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
        .indexed_statements(state.statement_nodes)
    {
        // Check reads before this statement changes storage. Specialized
        // callee checks may suppress ordinary diagnostics, so they need these
        // checks even when they do not collect additional crash records.
        scan_statement_reads(
            program,
            machine,
            state,
            statement,
            statement_ordinal,
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
                    refuse_open_windows(
                        &tracked,
                        &inherited_windows,
                        machine,
                        state,
                        Some(statement_ordinal),
                        "a call",
                        diagnostics,
                    );
                    preserve_proven_establishment(&tracked, &mut call_established);
                    tracked.clear();
                    poisoned_all = true;
                    collect_call_summaries(
                        program,
                        assignment.value,
                        state_to_machine,
                        summaries,
                        &mut call_established,
                    );
                    for index in &index_expressions {
                        collect_call_summaries(
                            program,
                            *index,
                            state_to_machine,
                            summaries,
                            &mut call_established,
                        );
                    }
                }
                handle_assignment(
                    program,
                    machine,
                    state,
                    statement,
                    call_frames,
                    &mut tracked,
                    entry_valuations,
                    &inherited_windows,
                    poisoned_all,
                    &mut poisoned_paths,
                    born_zero,
                    statement_ordinal,
                    diagnostics,
                );
                // Assignment checking establishes the destination's declared
                // type, not an unfinished containing predicate or every
                // element a runtime selector could identify. Retain this for
                // scalar as well as nominal destinations; windows are checked
                // independently and may still forbid observing the parent.
                if let Some(spelling) =
                    place_queries::write_place_spelling(program, assignment.target)
                    && !spelling.contains("[*]")
                    && !call_established.contains(&spelling)
                {
                    call_established.push(spelling);
                }
                // A write that re-proved the facts (tracked window CLOSED)
                // closes the inherited window on the same place.
                inherited_windows.retain(|(spelling, _, _)| {
                    !tracked
                        .iter()
                        .any(|place| place.spelling == *spelling && !place.any_window_open())
                });
            }
            // A call OBSERVES state, so it remains a hard consumption point
            // (ch11): every open window must have closed. After that check,
            // R5 summaries preserve exact valuations outside known writes.
            StatementNode::Call(call) => {
                let saved_places = place_snapshots(&tracked);
                refuse_open_windows(
                    &tracked,
                    &inherited_windows,
                    machine,
                    state,
                    Some(statement_ordinal),
                    "a call",
                    diagnostics,
                );
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
                let target_machine = state_to_machine
                    .get(&call.target_symbol)
                    .copied()
                    .unwrap_or_else(symbols::SymbolHandle::invalid);
                if let Some(established) = summaries.get(&target_machine) {
                    call_established.extend(established.iter().cloned());
                }
                restore_parameter_write(
                    program,
                    call_frames,
                    parameter_call_attempts,
                    call.target_symbol,
                    program.expression_table.expression_handles(call.arguments),
                    state_to_machine,
                    &saved_places,
                    &mut tracked,
                    diagnostics,
                    active_callees,
                );
            }
            StatementNode::Expression(expression) => {
                let saved_places = place_snapshots(&tracked);
                if expression_contains_call(program, *expression) {
                    refuse_open_windows(
                        &tracked,
                        &inherited_windows,
                        machine,
                        state,
                        Some(statement_ordinal),
                        "a call",
                        diagnostics,
                    );
                    preserve_proven_establishment(&tracked, &mut call_established);
                    tracked.clear();
                    poisoned_all = true;
                    collect_call_summaries(
                        program,
                        *expression,
                        state_to_machine,
                        summaries,
                        &mut call_established,
                    );
                }
                if let typed_trees::expression::ExpressionNode::Call(call) =
                    program.expression_table.expression(*expression)
                {
                    restore_parameter_write(
                        program,
                        call_frames,
                        parameter_call_attempts,
                        call.target_symbol,
                        program.expression_table.expression_handles(call.arguments),
                        state_to_machine,
                        &saved_places,
                        &mut tracked,
                        diagnostics,
                        active_callees,
                    );
                }
            }
            StatementNode::LocalData(local) => {
                let saved_places = place_snapshots(&tracked);
                if local.initial_value.is_valid()
                    && expression_contains_call(program, local.initial_value)
                {
                    refuse_open_windows(
                        &tracked,
                        &inherited_windows,
                        machine,
                        state,
                        Some(statement_ordinal),
                        "a call",
                        diagnostics,
                    );
                    preserve_proven_establishment(&tracked, &mut call_established);
                    tracked.clear();
                    poisoned_all = true;
                    collect_call_summaries(
                        program,
                        local.initial_value,
                        state_to_machine,
                        summaries,
                        &mut call_established,
                    );
                    let mut calls = Vec::new();
                    visit_calls(program, local.initial_value, &mut calls);
                    for call in calls {
                        restore_parameter_write(
                            program,
                            call_frames,
                            parameter_call_attempts,
                            call.target_symbol,
                            program.expression_table.expression_handles(call.arguments),
                            state_to_machine,
                            &saved_places,
                            &mut tracked,
                            diagnostics,
                            active_callees,
                        );
                    }
                }
                // Initializer admission remains the ordinary construction/call
                // obligation. Retain its independently completed case at this
                // cut; a later payload write cannot rediscover it from syntax
                // or the destination annotation after the source has changed.
                let literal_case = local
                    .initial_value
                    .is_valid()
                    .then(
                        || match program.expression_table.expression(local.initial_value) {
                            typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
                                literal.case_symbol.filter(|symbol| symbol.is_valid())
                            }
                            _ => None,
                        },
                    )
                    .flatten();
                if local.initial_value.is_valid()
                    && let Some(definition) = crate::field_domains::owned_nominal_data_definition(program, local.type_reference)
                    && program.data_members(definition).iter().any(|member| member.as_variant().is_some())
                    && !tracked.iter().any(|place| place.spelling == local.name.as_str())
                    && let Some(active_case) = literal_case.or_else(|| {
                        crate::proof_contracts::contract_entailment::completed_value_case_at_occurrence(
                            program, machine, state, statement_ordinal, local.initial_value, local.type_reference,
                        )
                    })
                {
                    let (fields, symbols) = symbolic_values::struct_literal_snapshot(
                        program,
                        local.initial_value,
                    );
                tracked.push(TrackedPlace {
                        spelling: local.name.to_string(), definition,
                        fields, established_fields: established_common_fields(program, definition), pending_requirements: Vec::new(), symbols, measures: Vec::new(),
                        established: true, born_zero: false, window_open: false, opening_write: None,
                        elements: Vec::new(), element_windows: Vec::new(), active_case: Some(active_case),
                        alternatives: Vec::new(),
                    });
                }
                seed_transported_initializer(program, local, &mut tracked);
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
                            .filter(|place| place.any_window_open())
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
                let saved_places = place_snapshots(&tracked);
                if evaluated
                    .iter()
                    .any(|expression| expression_contains_call(program, *expression))
                {
                    refuse_open_windows(
                        &tracked,
                        &inherited_windows,
                        machine,
                        state,
                        Some(statement_ordinal),
                        "a call",
                        diagnostics,
                    );
                    preserve_proven_establishment(&tracked, &mut call_established);
                    tracked.clear();
                    poisoned_all = true;
                    for expression in &evaluated {
                        collect_call_summaries(
                            program,
                            *expression,
                            state_to_machine,
                            summaries,
                            &mut call_established,
                        );
                    }
                }
                let mut calls = Vec::new();
                for expression in &evaluated {
                    visit_calls(program, *expression, &mut calls);
                }
                for call in calls {
                    restore_parameter_write(
                        program,
                        call_frames,
                        parameter_call_attempts,
                        call.target_symbol,
                        program.expression_table.expression_handles(call.arguments),
                        state_to_machine,
                        &saved_places,
                        &mut tracked,
                        diagnostics,
                        active_callees,
                    );
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
        refuse_open_windows(
            &tracked,
            &inherited_windows,
            machine,
            state,
            None,
            "return",
            diagnostics,
        );
    }

    let mut exit_established: Vec<String> = entry_established.to_vec();
    // Local validity survives valuation poisoning within this invocation, not
    // as a same-spelled fact about another state's per-invocation local.
    exit_established.extend(
        call_established
            .iter()
            .filter(|place| is_data_rooted(data_binding, place))
            .cloned(),
    );
    exit_established.extend(
        tracked
            .iter()
            // Slice 6: parameters are per-invocation -- only machine-owned
            // places transport across states.
            .filter(|place| place.established && is_data_rooted(data_binding, &place.spelling))
            .map(|place| place.spelling.clone()),
    );
    if let Some(data_binding) = data_binding
        && attached_value_established(
            program,
            machine,
            state,
            &tracked,
            entry_established,
            &call_established,
        )
    {
        exit_established.push(data_binding.to_owned());
    }
    exit_established.sort();
    exit_established.dedup();

    // Exit valuations: in-state tracked places, plus entry places that no
    // known write overlaps. An opaque call poisons every untouched entry.
    let mut exit_valuations: Vec<PlaceValuation> = tracked
        .iter()
        .filter(|place| is_data_rooted(data_binding, &place.spelling))
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
                .any(|written| place_queries::place_spellings_may_overlap(spelling, written));
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
        .filter(|place| place.any_window_open() && is_data_rooted(data_binding, &place.spelling))
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

//! Whole-call-graph worst-case stack usage derivation for one task
//! activation.
//!
//! A frame is one machine activation entered at an exact checked entry state.
//! Intra-machine transitions extend the activation's resident state set
//! without adding a frame; an ordinary call to a checked-body machine places
//! a child frame on this same stack. Calls whose targets resolve to
//! requirement, boundary, admission-claim or external supply are
//! provider-domain transfers, and calls whose targets name no machine state
//! at all — requirement slots, machine parameters, dynamic descriptors —
//! carry no checked frame either. Neither kind is dropped: each enters the
//! frame's `UnresolvedCallSite` roster, so the composed demand publishes as
//! partial until provider admission covers the site — binding it to a
//! checked-body callee subtree through `CallTargetBinding`, moving it to an
//! admitted same-stack contribution, or proving the transfer off this stack.
//!
//! A frame's local demand is its worst-case resident extent while any child
//! runs: the resume-state word, every non-erased state parameter and local
//! binding of every reachable state, and the staged call-argument carry recorded at that state's
//! suspension crossings. Erased proof bindings own no runtime storage and are
//! skipped the way layout skips them. Liveness-exact slot packing belongs to
//! physical frame realization; this derivation is the conservative exact
//! bound over retained checked evidence.
//!
//! Every possibly-suspending call in the reachable graph must carry a
//! canonical suspension crossing. A missing row rejects here so coordinated
//! deletion of a Terminal site and its plan cannot erase required crossing
//! demand.

use super::carry_crossings::{exact_activation_wide_carry, validate_activation_carry_crossing};
use checked_trees::SuspensionCrossingStorage;
use diagnostics::Diagnostic;
use language_semantics::{CarryPolicy, MachineSupplyMode};
use symbols::SymbolHandle;
use terminal_psi::{
    TerminalIdentityTableBuilder, TerminalTaskFrameCall, TerminalTaskUnresolvedCall,
    TerminalTaskUnresolvedCallKind as UnresolvedCallKind,
};
use typed_trees::types::TypeReferenceHandle;

pub(crate) struct TaskFrameState {
    pub(crate) state: terminal_psi::TerminalIdentityIndex,
    pub(crate) resident: Vec<TypeReferenceHandle>,
    pub(crate) crossing_arguments: Vec<Vec<TypeReferenceHandle>>,
}
pub(crate) struct TaskFrame {
    pub(crate) identity: u64,
    pub(crate) machine_callable_identity: terminal_psi::TerminalIdentityIndex,
    pub(crate) entry: terminal_psi::TerminalIdentityIndex,
    pub(crate) states: Vec<TaskFrameState>,
    pub(crate) calls: Vec<TerminalTaskFrameCall>,
    pub(crate) unresolved_calls: Vec<TerminalTaskUnresolvedCall>,
}

/// The reachable checked call graph of one task activation, expressed as the
/// portable state/call obligations plus the suspension crossings reached inside
/// that graph. Omega supplies geometry and composes the physical stack bound.
pub(crate) struct TaskCallGraph<'program> {
    pub(crate) root: u64,
    pub(crate) frames: Vec<TaskFrame>,
    /// Canonical crossing rows whose `(machine, state)` belongs to a reachable
    /// frame, including crossings inside nested checked callees.
    pub(crate) crossings: Vec<&'program checked_trees::SuspensionCrossingCarryFact>,
    /// Direct envelopes combined over these same reached machine frames.
    pub(crate) carry: CarryPolicy,
}

/// Derive the reachable same-stack call graph of the activation entered at
/// `(root_machine, root_entry)`. Source coordinates are checked here;
/// source-free native realization seals physical frame and stack composition.
pub(crate) fn task_call_graph<'program>(
    typed: &'program checked_trees::typed_trees::TypedTrees,
    check_facts: &'program checked_trees::CheckFacts,
    identities: &mut TerminalIdentityTableBuilder,
    root_machine: &'program checked_trees::machine::Machine,
    root_entry: &'program checked_trees::state::State,
) -> Result<TaskCallGraph<'program>, Vec<Diagnostic>> {
    let root = frame_identity(typed, root_machine, root_entry)?;
    let mut pending = vec![root_entry.symbol];
    let mut visited = Vec::new();
    let mut frames = Vec::new();
    let mut crossings = Vec::new();
    let mut carry = CarryPolicy::PERMISSIVE;
    while let Some(entry_symbol) = pending.pop() {
        if visited.contains(&entry_symbol) {
            continue;
        }
        visited.push(entry_symbol);
        let (machine, entry) = exact_frame_target(typed, entry_symbol)?;
        carry = carry.intersect(
            exact_activation_wide_carry(check_facts, machine.symbol, machine.name.as_str())?
                .effective,
        );
        let summary = task_frame_summary(
            typed,
            check_facts,
            identities,
            machine,
            entry,
            &mut pending,
            &mut crossings,
        )?;
        frames.push(summary);
    }
    Ok(TaskCallGraph {
        root,
        frames,
        crossings,
        carry,
    })
}

/// Resolve an entry state symbol to its exact owning machine and state. A
/// state symbol is globally unique, so a zero or plural resolution means the
/// checked program retained a drifted call target and must fail closed.
pub(crate) fn exact_frame_target<'program>(
    typed: &'program checked_trees::typed_trees::TypedTrees,
    entry: SymbolHandle,
) -> Result<
    (
        &'program checked_trees::machine::Machine,
        &'program checked_trees::state::State,
    ),
    Vec<Diagnostic>,
> {
    let mut matches = typed.machine_states_named(entry);
    let (machine, state) = matches.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "task WCSU graph call target must name an exact typed state",
        )]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(
            "task WCSU graph call target must resolve to exactly one typed state",
        )]);
    }
    Ok((machine, state))
}

/// Build one frame's obligations: its resident types and checked call edges.
/// Callee frames discovered through checked calls are pushed onto `pending`
/// so the traversal covers the complete reachable graph.
fn task_frame_summary<'program>(
    typed: &'program checked_trees::typed_trees::TypedTrees,
    check_facts: &'program checked_trees::CheckFacts,
    identities: &mut TerminalIdentityTableBuilder,
    machine: &'program checked_trees::machine::Machine,
    entry: &'program checked_trees::state::State,
    pending: &mut Vec<SymbolHandle>,
    reachable_crossings: &mut Vec<&'program checked_trees::SuspensionCrossingCarryFact>,
) -> Result<TaskFrame, Vec<Diagnostic>> {
    let frame = frame_identity(typed, machine, entry)?;
    let states = frame_state_closure(typed, machine, entry)?;

    let mut resident_states = Vec::new();
    for state in &states {
        let mut resident = typed
            .state_parameters(state)
            .iter()
            .filter(|parameter| !parameter.relevance.is_erased())
            .map(|parameter| parameter.type_reference)
            .collect::<Vec<_>>();
        for statement in typed.statement_table.statements(state.statement_nodes) {
            if let Some(local) = statement.as_local_data()
                && !local.relevance.is_erased()
            {
                resident.push(local.type_reference);
            }
        }
        let mut crossing_coordinates = Vec::new();
        let mut crossing_arguments = Vec::new();
        for crossing in check_facts
            .carry
            .suspension_crossings
            .iter()
            .filter(|crossing| crossing.machine == machine.symbol && crossing.state == state.symbol)
        {
            validate_activation_carry_crossing(typed, check_facts, crossing)?;
            let coordinate = (crossing.statement_index, crossing.call_ordinal);
            if crossing_coordinates.contains(&coordinate) {
                return Err(vec![Diagnostic::error(
                    "task activation carry crossings must retain one row per exact call coordinate",
                )]);
            }
            crossing_coordinates.push(coordinate);
            reachable_crossings.push(crossing);
            crossing_arguments.push(
                crossing
                    .live_values
                    .iter()
                    .filter(|live| live.storage == SuspensionCrossingStorage::CallArgument)
                    .map(|live| live.type_reference)
                    .collect(),
            );
        }
        resident_states.push(TaskFrameState {
            state: identities.intern(state.name.as_str()),
            resident,
            crossing_arguments,
        });
    }

    let mut calls = Vec::new();
    let mut unresolved_calls = Vec::new();
    for state in &states {
        let mut flow_states = check_facts.flow.control.states.iter().filter(|(_, flow)| {
            flow.machine_symbol == machine.symbol && flow.state_symbol == state.symbol
        });
        let Some((_, flow_state)) = flow_states.next() else {
            continue;
        };
        if flow_states.next().is_some() {
            return Err(vec![Diagnostic::error(
                "task WCSU frame must name exactly one checked flow state",
            )]);
        }
        let flow_calls = check_facts
            .flow
            .control
            .calls
            .span(flow_state.calls)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "task WCSU frame flow state must retain an exact valid call span",
                )]
            })?;
        for call in flow_calls {
            // A retired row was replaced by selected execution in the typed
            // body; execution planning skips it, so it places no checked frame.
            if check_facts.flow.control.is_retired(state.symbol, call) {
                continue;
            }
            let may_suspend =
                call.suspension.direct_may_suspend || call.suspension.transitive_may_suspend;
            if may_suspend
                && !check_facts
                    .carry
                    .suspension_crossings
                    .iter()
                    .any(|crossing| {
                        crossing.machine == machine.symbol
                            && crossing.state == state.symbol
                            && crossing.statement_index == call.statement_index
                            && crossing.call_ordinal == call.call_ordinal
                    })
            {
                return Err(vec![Diagnostic::error(format!(
                    "task WCSU frame `{}` state `{}` has a possibly-suspending call at statement {} call {} with no canonical suspension crossing",
                    machine.name, state.name, call.statement_index, call.call_ordinal,
                ))]);
            }
            let Some((callee_machine, callee_entry)) =
                checked_call_target(typed, call.target_symbol)?
            else {
                // Requirement slots, machine parameters and dynamic
                // descriptor calls name no machine state. The bound cannot
                // pretend their same-stack demand is zero; record the site so
                // the composed demand publishes as partial until provider
                // admission covers it.
                unresolved_calls.push(TerminalTaskUnresolvedCall {
                    state: identities.intern(state.name.as_str()),
                    statement_index: u32::try_from(call.statement_index).map_err(|_| {
                        vec![Diagnostic::error(
                            "task call statement coordinate exceeds u32",
                        )]
                    })?,
                    call_ordinal: u32::try_from(call.call_ordinal)
                        .map_err(|_| vec![Diagnostic::error("task call ordinal exceeds u32")])?,
                    kind: UnresolvedCallKind::UnresolvedTarget,
                });
                continue;
            };
            if callee_machine.supply_mode != MachineSupplyMode::CheckedBody {
                // Requirement, boundary, admission-claim and external supply
                // resolve to a machine state but not to a checked body this
                // stack can bound. Same accountability: the site is
                // unresolved until provider admission binds its demand.
                unresolved_calls.push(TerminalTaskUnresolvedCall {
                    state: identities.intern(state.name.as_str()),
                    statement_index: u32::try_from(call.statement_index).map_err(|_| {
                        vec![Diagnostic::error(
                            "task call statement coordinate exceeds u32",
                        )]
                    })?,
                    call_ordinal: u32::try_from(call.call_ordinal)
                        .map_err(|_| vec![Diagnostic::error("task call ordinal exceeds u32")])?,
                    kind: UnresolvedCallKind::NonCheckedSupply,
                });
                continue;
            }
            let callee = frame_identity(typed, callee_machine, callee_entry)?;
            calls.push(TerminalTaskFrameCall { callee });
            pending.push(callee_entry.symbol);
        }
    }

    Ok(TaskFrame {
        identity: frame,
        machine_callable_identity: identities.intern(
            &typed
                .machine_callable_identity_text(machine)
                .ok_or_else(|| vec![Diagnostic::error("task frame lacks its callable identity")])?,
        ),
        entry: identities.intern(entry.name.as_str()),
        states: resident_states,
        calls,
        unresolved_calls,
    })
}

/// The resident state set of one activation: the entry state plus every state
/// reached by intra-machine transitions. Named targets resolve inside the
/// owning machine only; a target outside the state roster is not a transition
/// of this activation and contributes no state.
fn frame_state_closure<'program>(
    typed: &'program checked_trees::typed_trees::TypedTrees,
    machine: &'program checked_trees::machine::Machine,
    entry: &'program checked_trees::state::State,
) -> Result<Vec<&'program checked_trees::state::State>, Vec<Diagnostic>> {
    let mut ordered = Vec::new();
    let mut seen = Vec::new();
    let mut pending = vec![entry.symbol];
    while let Some(symbol) = pending.pop() {
        if seen.contains(&symbol) {
            continue;
        }
        seen.push(symbol);
        let state = typed.state_in_machine(machine, symbol).ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "task WCSU frame `{}` transition reaches a state outside its machine",
                machine.name
            ))]
        })?;
        for statement in typed.statement_table.statements(state.statement_nodes) {
            let Some(transition) = statement.as_transition() else {
                continue;
            };
            push_transition_successor(typed, machine, transition.target, &mut pending);
            if transition.continuation.is_valid() {
                push_transition_successor(typed, machine, transition.continuation, &mut pending);
            }
        }
        ordered.push(state);
    }
    Ok(ordered)
}

fn push_transition_successor<'program>(
    typed: &'program checked_trees::typed_trees::TypedTrees,
    machine: &'program checked_trees::machine::Machine,
    target: checked_trees::typed_trees::statement::TransitionTargetHandle,
    pending: &mut Vec<SymbolHandle>,
) {
    let symbol = match typed.statement_table.transition_target(target) {
        checked_trees::typed_trees::statement::TransitionTargetNode::Named { path, .. } => {
            path.symbol
        }
        checked_trees::typed_trees::statement::TransitionTargetNode::Value(_)
        | checked_trees::typed_trees::statement::TransitionTargetNode::Terminal => return,
    };
    if symbol.is_valid()
        && typed
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == symbol)
    {
        pending.push(symbol);
    }
}

/// Resolve a checked call target to its exact callee machine and entry state.
/// Targets that name no machine state -- requirement slots, machine
/// parameters, dynamic descriptor calls -- return `None`; they place no
/// checked frame on this stack.
fn checked_call_target<'program>(
    typed: &'program checked_trees::typed_trees::TypedTrees,
    target: SymbolHandle,
) -> Result<
    Option<(
        &'program checked_trees::machine::Machine,
        &'program checked_trees::state::State,
    )>,
    Vec<Diagnostic>,
> {
    let mut matches = typed.machine_states_named(target);
    let Some(target) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(
            "task WCSU graph call target must resolve to exactly one typed state",
        )]);
    }
    Ok(Some(target))
}

/// The stable identity of one frame: the machine's normalized callable
/// overload joined with the exact entry state name. Symbol handles are
/// per-compilation and never enter a normalized plan identity.
fn frame_identity<'program>(
    typed: &'program checked_trees::typed_trees::TypedTrees,
    machine: &'program checked_trees::machine::Machine,
    entry: &'program checked_trees::state::State,
) -> Result<u64, Vec<Diagnostic>> {
    let callable = typed
        .machine_callable_identity_text(machine)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "task WCSU frame `{}` has no normalized machine overload identity",
                machine.name
            ))]
        })?;
    let mut hash = semantic_vocabulary::Fnv1a::new();
    hash.byte(0x57);
    hash.string_terminated(callable.as_str());
    hash.string_terminated(entry.name.as_str());
    Ok(hash.finish_or(1))
}

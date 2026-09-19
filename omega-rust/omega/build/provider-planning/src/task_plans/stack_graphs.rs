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
//! runs: the resume-state word, the machine's own persistent storage layout,
//! every non-erased state parameter and local binding of every reachable
//! state, and the staged call-argument carry recorded at that state's
//! suspension crossings. Erased proof bindings own no runtime storage and are
//! skipped the way layout skips them. Liveness-exact slot packing belongs to
//! physical frame realization; this derivation is the conservative exact
//! bound over retained checked evidence.
//!
//! Every possibly-suspending call in the reachable graph must carry a
//! canonical suspension crossing. A missing row rejects here so coordinated
//! deletion of a Terminal site and its plan cannot erase required crossing
//! demand.

use checked_trees::{CheckedTrees, SuspensionCrossingStorage};
use diagnostics::Diagnostic;
use language_semantics::MachineSupplyMode;
use layout::TypeLayout;
use symbols::SymbolHandle;
use target::NativeTarget;
use task_plans::{
    StackCallContribution, TaskStackFrameId, TaskStackFrameSummary, UnresolvedCallKind,
    UnresolvedCallSite, ValidatedTaskStackFrameSummary, task_stack_frame_validation_identity,
    validate_task_stack_frame_summary,
};

/// The reachable checked call graph of one task activation, expressed as the
/// validated frame summaries `compose_task_stack_demand` consumes, plus the
/// suspension crossings reached inside that graph.
pub(crate) struct TaskCallGraph<'program> {
    pub(crate) root: TaskStackFrameId,
    pub(crate) frames: Vec<ValidatedTaskStackFrameSummary>,
    /// Canonical crossing rows whose `(machine, state)` belongs to a reachable
    /// frame, including crossings inside nested checked callees. The plan's
    /// authoritative roster unions these with the containment subtree rows.
    pub(crate) crossings: Vec<&'program checked_trees::SuspensionCrossingCarryFact>,
}

/// Derive the reachable same-stack call graph of the activation entered at
/// `(root_machine, root_entry)`. The returned summaries are individually
/// validated; graph-level closure, reachability and acyclicity are sealed by
/// `compose_task_stack_demand`.
pub(crate) fn task_call_graph<'program>(
    program: &'program CheckedTrees,
    target: NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    layouts: &layout::LayoutPlan,
    root_machine: &checked_trees::machine::Machine,
    root_entry: &checked_trees::state::State,
) -> Result<TaskCallGraph<'program>, Vec<Diagnostic>> {
    let root = frame_identity(program, root_machine, root_entry)?;
    let mut pending = vec![root_entry.symbol];
    let mut visited = Vec::new();
    let mut frames = Vec::new();
    let mut crossings = Vec::new();
    while let Some(entry_symbol) = pending.pop() {
        if visited.contains(&entry_symbol) {
            continue;
        }
        visited.push(entry_symbol);
        let (machine, entry) = exact_frame_target(program, entry_symbol)?;
        let summary = task_frame_summary(
            program,
            target,
            opaque_representation_selections,
            layouts,
            machine,
            entry,
            &mut pending,
            &mut crossings,
        )?;
        frames.push(
            validate_task_stack_frame_summary(summary)
                .map_err(|error| vec![Diagnostic::error(error.to_string())])?,
        );
    }
    Ok(TaskCallGraph {
        root,
        frames,
        crossings,
    })
}

/// Resolve an entry state symbol to its exact owning machine and state. A
/// state symbol is globally unique, so a zero or plural resolution means the
/// checked program retained a drifted call target and must fail closed.
fn exact_frame_target(
    program: &CheckedTrees,
    entry: SymbolHandle,
) -> Result<
    (
        &checked_trees::machine::Machine,
        &checked_trees::state::State,
    ),
    Vec<Diagnostic>,
> {
    let mut matches = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .map(move |state| (machine, state))
            .filter(|(_, state)| state.symbol == entry)
    });
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

/// Build one frame's summary: its resident extent and its checked call edges.
/// Callee frames discovered through checked calls are pushed onto `pending`
/// so the traversal covers the complete reachable graph.
fn task_frame_summary<'program>(
    program: &'program CheckedTrees,
    target: NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    layouts: &layout::LayoutPlan,
    machine: &'program checked_trees::machine::Machine,
    entry: &'program checked_trees::state::State,
    pending: &mut Vec<SymbolHandle>,
    reachable_crossings: &mut Vec<&'program checked_trees::SuspensionCrossingCarryFact>,
) -> Result<TaskStackFrameSummary, Vec<Diagnostic>> {
    let frame = frame_identity(program, machine, entry)?;
    let states = frame_state_closure(program, machine, entry)?;

    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find_map(|(_, layout)| (layout.symbol == machine.symbol).then_some(layout.layout))
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "task WCSU frame `{}` has no concrete machine layout",
                machine.name
            ))]
        })?;
    // Every frame keeps an explicit resume-state word so a parked live chain
    // retains where each caller resumes, even when the machine stores nothing.
    let state_word = TypeLayout {
        size: target.pointer_size,
        alignment: target.pointer_alignment,
    };
    let mut base_size = 0usize;
    let mut base_alignment = 1usize;
    append_layout(&mut base_size, &mut base_alignment, state_word)?;
    append_layout(&mut base_size, &mut base_alignment, machine_layout)?;
    base_size = align_to(base_size, base_alignment)?;

    let mut local_bytes = base_size;
    let mut local_alignment = base_alignment;
    for state in &states {
        let mut size = base_size;
        let mut alignment = base_alignment;
        for parameter in program
            .state_parameters(state)
            .iter()
            .filter(|parameter| !parameter.relevance.is_erased())
        {
            append_layout(
                &mut size,
                &mut alignment,
                frame_value_layout(
                    program,
                    target,
                    opaque_representation_selections,
                    parameter.type_reference,
                )?,
            )?;
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            let typed_trees::statement::StatementNode::LocalData(local) = statement else {
                continue;
            };
            if local.relevance.is_erased() {
                continue;
            }
            append_layout(
                &mut size,
                &mut alignment,
                frame_value_layout(
                    program,
                    target,
                    opaque_representation_selections,
                    local.type_reference,
                )?,
            )?;
        }
        // Only one crossing can be parked at a time, so staged call-argument
        // demand composes by maximum across this state's crossings.
        let mut state_peak = align_to(size, alignment)?;
        for crossing in program
            .facts
            .carry
            .suspension_crossings
            .iter()
            .filter(|crossing| crossing.machine == machine.symbol && crossing.state == state.symbol)
        {
            reachable_crossings.push(crossing);
            let mut crossing_size = size;
            let mut crossing_alignment = alignment;
            for live in &crossing.live_values {
                if live.storage != SuspensionCrossingStorage::CallArgument {
                    continue;
                }
                append_layout(
                    &mut crossing_size,
                    &mut crossing_alignment,
                    frame_value_layout(
                        program,
                        target,
                        opaque_representation_selections,
                        live.type_reference,
                    )?,
                )?;
            }
            state_peak = state_peak.max(align_to(crossing_size, crossing_alignment)?);
        }
        local_bytes = local_bytes.max(state_peak);
        local_alignment = local_alignment.max(alignment);
    }

    let mut calls = Vec::new();
    let mut unresolved_calls = Vec::new();
    for state in &states {
        let mut flow_states = program
            .facts
            .flow
            .control
            .states
            .iter()
            .filter(|(_, flow)| {
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
        let flow_calls = program
            .facts
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
            if program.facts.flow.control.is_retired(state.symbol, call) {
                continue;
            }
            let may_suspend =
                call.suspension.direct_may_suspend || call.suspension.transitive_may_suspend;
            if may_suspend
                && !program
                    .facts
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
                checked_call_target(program, call.target_symbol)?
            else {
                // Requirement slots, machine parameters and dynamic
                // descriptor calls name no machine state. The bound cannot
                // pretend their same-stack demand is zero; record the site so
                // the composed demand publishes as partial until provider
                // admission covers it.
                unresolved_calls.push(UnresolvedCallSite {
                    frame,
                    state: state.name.as_str().into(),
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    kind: UnresolvedCallKind::UnresolvedTarget,
                });
                continue;
            };
            if callee_machine.supply_mode != MachineSupplyMode::CheckedBody {
                // Requirement, boundary, admission-claim and external supply
                // resolve to a machine state but not to a checked body this
                // stack can bound. Same accountability: the site is
                // unresolved until provider admission binds its demand.
                unresolved_calls.push(UnresolvedCallSite {
                    frame,
                    state: state.name.as_str().into(),
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    kind: UnresolvedCallKind::NonCheckedSupply,
                });
                continue;
            }
            let callee = frame_identity(program, callee_machine, callee_entry)?;
            calls.push(StackCallContribution::Checked { callee });
            pending.push(callee_entry.symbol);
        }
    }

    Ok(TaskStackFrameSummary {
        frame,
        local_bytes: u64::try_from(local_bytes)
            .map_err(|_| vec![Diagnostic::error("task WCSU frame size exceeds u64")])?,
        alignment: u64::try_from(local_alignment)
            .map_err(|_| vec![Diagnostic::error("task WCSU frame alignment exceeds u64")])?,
        validation: task_stack_frame_validation_identity(
            frame,
            u64::try_from(local_bytes)
                .map_err(|_| vec![Diagnostic::error("task WCSU frame size exceeds u64")])?,
            u64::try_from(local_alignment)
                .map_err(|_| vec![Diagnostic::error("task WCSU frame alignment exceeds u64")])?,
            &calls,
            &unresolved_calls,
        ),
        calls,
        unresolved_calls,
    })
}

/// The resident state set of one activation: the entry state plus every state
/// reached by intra-machine transitions. Named targets resolve inside the
/// owning machine only; a target outside the state roster is not a transition
/// of this activation and contributes no state.
fn frame_state_closure<'program>(
    program: &'program CheckedTrees,
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
        let state = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "task WCSU frame `{}` transition reaches a state outside its machine",
                    machine.name
                ))]
            })?;
        for statement in program.statement_table.statements(state.statement_nodes) {
            let typed_trees::statement::StatementNode::Transition(transition) = statement else {
                continue;
            };
            push_transition_successor(program, machine, state, transition.target, &mut pending);
            if transition.continuation.is_valid() {
                push_transition_successor(
                    program,
                    machine,
                    state,
                    transition.continuation,
                    &mut pending,
                );
            }
        }
        ordered.push(state);
    }
    Ok(ordered)
}

fn push_transition_successor(
    program: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    target: typed_trees::statement::TransitionTargetHandle,
    pending: &mut Vec<SymbolHandle>,
) {
    let symbol = match program.statement_table.transition_target(target) {
        typed_trees::statement::TransitionTargetNode::Named { path, .. } => path.symbol,
        typed_trees::statement::TransitionTargetNode::SelfTarget => state.symbol,
        typed_trees::statement::TransitionTargetNode::Value(_)
        | typed_trees::statement::TransitionTargetNode::Terminal => return,
    };
    if symbol.is_valid()
        && program
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
fn checked_call_target(
    program: &CheckedTrees,
    target: SymbolHandle,
) -> Result<
    Option<(
        &checked_trees::machine::Machine,
        &checked_trees::state::State,
    )>,
    Vec<Diagnostic>,
> {
    let mut matches = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .map(move |state| (machine, state))
            .filter(|(_, state)| state.symbol == target)
    });
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
fn frame_identity(
    program: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    entry: &checked_trees::state::State,
) -> Result<TaskStackFrameId, Vec<Diagnostic>> {
    let callable = program
        .normalized_machine_overload_identity(machine)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "task WCSU frame `{}` has no normalized machine overload identity",
                machine.name
            ))]
        })?;
    let mut hash = super::StableHash::new();
    hash.byte(0x57);
    hash.string(callable.identity().as_str());
    hash.string(entry.name.as_str());
    super::normalized_id(hash.finish(), TaskStackFrameId::from_normalized_identity)
}

fn frame_value_layout(
    program: &CheckedTrees,
    target: NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Result<TypeLayout, Vec<Diagnostic>> {
    layout::layout_type_reference(
        program,
        target,
        opaque_representation_selections,
        type_reference,
    )
    .map_err(|error| vec![error])
}

fn append_layout(
    size: &mut usize,
    alignment: &mut usize,
    layout: TypeLayout,
) -> Result<(), Vec<Diagnostic>> {
    let field_alignment = layout.alignment.max(1);
    *size = align_to(*size, field_alignment)?;
    *size = size
        .checked_add(layout.size)
        .ok_or_else(|| vec![Diagnostic::error("task WCSU frame layout size overflow")])?;
    *alignment = (*alignment).max(field_alignment);
    Ok(())
}

fn align_to(value: usize, alignment: usize) -> Result<usize, Vec<Diagnostic>> {
    value
        .checked_add(alignment.saturating_sub(1))
        .map(|rounded| rounded / alignment * alignment)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task WCSU frame layout alignment overflow",
            )]
        })
}

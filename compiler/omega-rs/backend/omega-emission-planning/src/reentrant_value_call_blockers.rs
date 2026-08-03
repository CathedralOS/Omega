use crate::EmissionPlanningInput;
use crate::blocker;
use crate::semantic_scope::state_name;
use omega_backend_report_types::EmissionBlocker;
use omega_control_flow::{ControlFlowPlan, MachineFlow, PlannedTransitionTarget, StateKey};
use omega_state_calls::StateCallRole;
use omega_state_storage::StateMutationKind;
use psi_arena::Arena;
use psi_symbols::SymbolHandle;

/// A VALUE-position call that reaches a RE-ENTRANT machine (one with a
/// transition back to its own ENTRY -- the canonical `terminates` walk)
/// whose looped body carries EFFECTS is a silent miscompile: the callee's
/// body ops are spliced ONCE into the caller's dispatch, outside the loop,
/// so per-iteration statement calls and mutations silently never re-execute
/// (probed 2026-07-07d: a separator-counting walk with a `self.bump(..)`
/// entry call delivered 0 natively vs 2 in the interpreter through three
/// shapes -- the walk as the DIRECT sibling callee, as a STATEMENT call
/// inside a value-called machine's entry, and as an ARM transition target).
///
/// Two shapes stay accepted, each pinned green by differential canaries:
/// - a PURE loop-carried recursion (`sum(n-1, acc+n)` with no body
///   statements -- calls/runtime_loop_accumulator_exit) delivers correctly
///   in value position on every face;
/// - a CONTAINED-object receiver (`self.r.sum(..)`) is a REAL machine call
///   into another instance's dispatch, which runs body effects properly
///   (the fs wrapper's host-calling entries; the dual-accumulator sample) --
///   so the fence skips the re-entrancy check on the contained target
///   ITSELF, but still walks its spliced interior: a sibling arm inside a
///   dispatched instance (`create_dir_all -> self.mkall_walk(..)`) splices
///   and breaks exactly like one at the root.
///
/// Statement-position calls to the same walks always work (recursion loops
/// via real state transitions). Overblocking is acceptable for a fence;
/// silent wrong values are not. The real fix is dispatch specialization
/// (call-with-return), which would retire this fence together with the
/// effectful-arm fence.
pub(crate) fn collect_reentrant_value_call_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    // The SELF/SIBLING/FREE continuum gets spliced inline; a named
    // contained-field receiver dispatches for real.
    let spliced_route = |receiver_name: &str| receiver_name.is_empty() || receiver_name == "self";

    let value_calls: Vec<_> = input
        .state_calls
        .calls
        .iter()
        .filter(|(_, call)| {
            call.reachable
                && call.target_key.machine.is_valid()
                && matches!(
                    call.role,
                    StateCallRole::AssignmentValue
                        | StateCallRole::CallArgument
                        | StateCallRole::TransitionArgument
                        | StateCallRole::TransitionGuard
                )
                // Dispatched calls are exempt (re-added 2026-07-09f, sound:
                // the call-result blocker guarantees a dispatched terminal is
                // served or refuses loudly -- see value_call_arm_effect_blockers
                // for the full rationale + the runtime differential proofs).
                && !crate::dispatch_route::state_call_routed_to_dispatch(input, call)
        })
        .map(|(_, call)| call)
        .collect();
    if value_calls.is_empty() {
        return;
    }

    // Call-graph edges: every reachable SPLICED-route call, regardless of
    // role -- a walk reached through a statement call inside a spliced body
    // miscompiles exactly like one reached through an arm target. Crossing
    // into a contained instance is a real dispatch and ends the walk.
    // NOTE: no `reachable` filter on edges or the effects scan -- a machine
    // reached only through an ARM target keeps its interior calls marked
    // unreachable, and the fence must still see them (the walkguard probe:
    // `count`'s arm targets `walk`, whose `self.bump(..)` entry call is the
    // very effect that breaks).
    let mut edges: Vec<(SymbolHandle, SymbolHandle)> = input
        .state_calls
        .calls
        .iter()
        .filter(|(_, call)| {
            call.target_key.machine.is_valid() && spliced_route(call.receiver_name.as_str())
        })
        .map(|(_, call)| (call.source_key.machine, call.target_key.machine))
        .collect();
    // ARM-target calls (`true -> self.walk(..)`) are TRANSITIONS, not
    // state-call records, and their `Nested.state_symbol` names the target's
    // ENTRY STATE symbol (`key.state`), not the machine symbol -- the same
    // resolution `branch_transition_target_key` performs. Overapproximate:
    // an edge to EVERY machine owning a state with that symbol (a fence may
    // overblock; it must never underblock).
    for (_, machine) in input.control_flow.machines.iter() {
        for state in input
            .control_flow
            .states
            .span(machine.states)
            .unwrap_or(&[])
        {
            for transition in input
                .control_flow
                .transitions
                .span(state.transitions)
                .unwrap_or(&[])
            {
                for target in [&transition.target, &transition.continuation] {
                    let PlannedTransitionTarget::Nested {
                        state_symbol,
                        receiver,
                        receiver_symbol,
                        ..
                    } = target
                    else {
                        continue;
                    };
                    if !state_symbol.is_valid() {
                        continue;
                    }
                    // Same receiver split as the call records: a contained
                    // Nested arm (`-> self.fs.op(..)`) dispatches for real.
                    if !(receiver.as_str() == "self"
                        || receiver.as_str().is_empty()
                        || *receiver_symbol == machine.symbol)
                    {
                        continue;
                    }
                    for (_, candidate) in input.control_flow.machines.iter() {
                        let owns_state = input
                            .control_flow
                            .states
                            .span(candidate.states)
                            .unwrap_or(&[])
                            .iter()
                            .any(|candidate_state| candidate_state.key.state == *state_symbol);
                        if owns_state {
                            edges.push((machine.symbol, candidate.symbol));
                        }
                    }
                }
            }
        }
    }

    let mut reported: Vec<(StateKey, usize)> = Vec::new();
    for call in value_calls {
        // A contained target is a real dispatch: its OWN recursion and body
        // effects run correctly there, so only its spliced interior is
        // checked. A spliced (self/sibling/free) target is checked itself.
        let skip_start = !spliced_route(call.receiver_name.as_str());
        let Some(reached) =
            first_broken_walk_reached(input, call.target_key.machine, skip_start, &edges)
        else {
            continue;
        };
        // One diagnostic per call site: several expansions/edges can reach
        // the same walk, and the message is identical for all of them.
        let site = (call.source_key, call.statement_index);
        if reported.contains(&site) {
            continue;
        }
        reported.push(site);

        let reached_name = input
            .control_flow
            .machine_by_symbol(reached)
            .map(|machine| machine.name.as_str().to_owned())
            .unwrap_or_else(|| "<unknown>".to_owned());
        blockers.insert(blocker(
            "state calls",
            &format!(
                "{} statement {}: a value call reaches the RE-ENTRANT machine \
                 `{}` (a `terminates` walk) whose looped body carries side \
                 effects (calls or writes) -- the spliced value route runs \
                 those effects at most ONCE, not per iteration, so the result \
                 would silently be wrong. Call the walking machine as a \
                 STATEMENT from a dispatched state and read its result from a \
                 field, or keep the loop body pure (loop-carried arguments \
                 only).",
                state_name(input, call.source_key),
                call.statement_index,
                reached_name,
            ),
        ));
    }
}

/// Breadth-first walk over the spliced-route call edges from `start`,
/// returning the first machine that both re-enters its entry AND carries
/// loop-body effects. `skip_start` exempts the start machine itself (the
/// contained-receiver real-dispatch case) while still walking its interior.
fn first_broken_walk_reached(
    input: &EmissionPlanningInput<'_>,
    start: SymbolHandle,
    skip_start: bool,
    edges: &[(SymbolHandle, SymbolHandle)],
) -> Option<SymbolHandle> {
    let mut visited = vec![start];
    let mut frontier = vec![start];
    while let Some(machine) = frontier.pop() {
        if !(skip_start && machine == start)
            && machine_reenters_entry(input.control_flow, machine)
            && machine_has_loop_body_effects(input, machine)
        {
            return Some(machine);
        }
        for (source, target) in edges {
            if *source == machine && !visited.contains(target) {
                visited.push(*target);
                frontier.push(*target);
            }
        }
    }
    None
}

/// Anything in the machine's body that must re-execute per loop iteration
/// (or per loop exit) to be correct: outgoing machine calls of any role,
/// host-boundary calls, machine-owned/param mutations, and sibling Nested
/// arm targets. A machine with none of these is a pure loop-carried
/// recursion, which every value route handles (its state is entirely in
/// transition arguments).
fn machine_has_loop_body_effects(
    input: &EmissionPlanningInput<'_>,
    machine_symbol: SymbolHandle,
) -> bool {
    if input
        .state_calls
        .calls
        .iter()
        .any(|(_, call)| call.source_key.machine == machine_symbol)
    {
        return true;
    }
    if input
        .host_calls
        .calls
        .iter()
        .any(|(_, host_call)| host_call.source_key.machine == machine_symbol)
    {
        return true;
    }
    if input.state_storage.mutations.iter().any(|(_, mutation)| {
        mutation.source_key.machine == machine_symbol
            && matches!(
                mutation.mutation_kind,
                StateMutationKind::MachineOwned | StateMutationKind::ParameterOrAlias
            )
    }) {
        return true;
    }
    let Some(machine) = input.control_flow.machine_by_symbol(machine_symbol) else {
        return false;
    };
    input
        .control_flow
        .states
        .span(machine.states)
        .unwrap_or(&[])
        .iter()
        .any(|state| {
            input
                .control_flow
                .transitions
                .span(state.transitions)
                .unwrap_or(&[])
                .iter()
                .any(|transition| {
                    matches!(transition.target, PlannedTransitionTarget::Nested { .. })
                        || matches!(
                            transition.continuation,
                            PlannedTransitionTarget::Nested { .. }
                        )
                })
        })
}

/// Mirrors the interpreter-visible recursion shape: any transition whose
/// target (or continuation) is the machine's ENTRY state. The canonical
/// `terminates` walk recursion is the entry transitioning to ITSELF
/// (`true -> walk(..)`), which control flow records as `SelfTarget`.
fn machine_reenters_entry(control_flow: &ControlFlowPlan, machine_symbol: SymbolHandle) -> bool {
    let Some(machine) = control_flow.machine_by_symbol(machine_symbol) else {
        return false;
    };
    machine_flow_reenters_entry(control_flow, machine)
}

fn machine_flow_reenters_entry(control_flow: &ControlFlowPlan, machine: &MachineFlow) -> bool {
    let Some(states) = control_flow.states.span(machine.states) else {
        return false;
    };
    let Some(entry) = states.first() else {
        return false;
    };
    states.iter().any(|state| {
        control_flow
            .transitions
            .span(state.transitions)
            .unwrap_or(&[])
            .iter()
            .any(|transition| {
                let targets_entry = |target: &PlannedTransitionTarget| match target {
                    PlannedTransitionTarget::State { key, .. } => *key == entry.key,
                    PlannedTransitionTarget::SelfTarget => state.key == entry.key,
                    _ => false,
                };
                targets_entry(&transition.target) || targets_entry(&transition.continuation)
            })
    })
}

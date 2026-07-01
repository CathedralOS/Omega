use crate::EmissionPlanningInput;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_state_calls::{StateCallLowering, StateCallRole};
use omega_state_storage::StateMutationKind;

use super::semantic_scope::{proof_scope_suffix, state_name};
use super::{EmissionBlocker, blocker};

/// Guards against the shared-result-slot silent miscompile.
///
/// A *pure computing* value-machine — one that returns a `let`-bound local and writes no
/// machine-owned field — called two or more times as a value in a single state makes every
/// call site read the LAST call's result. The cause: the callee's internal `let` slot is
/// allocated once per caller state and reused across the inlined call sites, and with no
/// intervening side effect the inliner defers all the result captures to after both bodies
/// have run, so they all read the final slot value. A machine-owned field write in the callee
/// forces sequential emission (each capture before the next call's body), which is exactly why
/// *stateful* computing machines are safe — and excluded here.
///
/// Until per-call-site slots exist, native emission would silently miscompile this shape, so we
/// turn it into a clean error. The sound workaround is one such value-call per state (compute a
/// value, transition, compute the next), which consumes each result before the next call runs.
pub(super) fn collect_shared_value_call_slot_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    // Collect value-position calls to a pure let-returning callee: (source, target, stmt, ordinal).
    let mut sites: Vec<(StateKey, StateKey, usize, usize)> = Vec::new();
    for (_, call) in input.state_calls.calls.iter() {
        if !call.required || !call.reachable || !call.target_key.is_valid() {
            continue;
        }
        // Only value-position calls capture a result into a slot; a plain `Statement` call is
        // sub-state control flow and never reads a callee result.
        if !matches!(
            call.role,
            StateCallRole::AssignmentValue
                | StateCallRole::CallArgument
                | StateCallRole::TransitionArgument
                | StateCallRole::TransitionGuard
        ) {
            continue;
        }
        // The shared slot only exists when the callee body is inlined into the caller state.
        if !matches!(
            call.lowering,
            StateCallLowering::InlineLeaf
                | StateCallLowering::InlineBranching
                | StateCallLowering::InlineExpansion
        ) {
            continue;
        }
        if !callee_is_pure_computing_machine(input, call.target_key) {
            continue;
        }
        sites.push((
            call.source_key,
            call.target_key,
            call.statement_index,
            call.call_ordinal,
        ));
    }

    // Flag any (source state, callee) pair reached by two or more distinct value-call sites.
    let mut reported: Vec<(StateKey, StateKey)> = Vec::new();
    for &(source, target, _, _) in &sites {
        if reported.iter().any(|&(s, t)| s == source && t == target) {
            continue;
        }
        let mut distinct: Vec<(usize, usize)> = sites
            .iter()
            .filter(|&&(s, t, _, _)| s == source && t == target)
            .map(|&(_, _, stmt, ordinal)| (stmt, ordinal))
            .collect();
        distinct.sort_unstable();
        distinct.dedup();
        if distinct.len() < 2 {
            continue;
        }
        reported.push((source, target));
        blockers.insert(blocker(
            "state calls",
            &format!(
                "{} makes {} value-calls to the computing machine {} in one state; native emission \
                 would share the callee's result slot across the call sites, so every site reads the \
                 last result. Use one such value-call per state (compute one value, transition, then \
                 compute the next){}",
                state_name(input, source),
                distinct.len(),
                state_name(input, target),
                proof_scope_suffix(input, source),
            ),
        ));
    }
}

/// A callee manifests the shared-slot bug iff it has an internal `let` local (the slot that is
/// shared) AND writes no machine-owned field anywhere in its body. Direct-return machines
/// (`-> param`, no local) and stateful machines (a `self.field` write forces sequential
/// emission) are both excluded — the first has no shared slot, the second's slot is safe.
fn callee_is_pure_computing_machine(input: &EmissionPlanningInput<'_>, target: StateKey) -> bool {
    let machine = target.machine;
    let has_local = input
        .state_storage
        .locals
        .iter()
        .any(|(_, local)| local.source_key.machine == machine);
    if !has_local {
        return false;
    }
    let writes_machine_owned_field = input
        .state_storage
        .mutations
        .iter()
        .any(|(_, mutation)| {
            mutation.source_key.machine == machine
                && matches!(mutation.mutation_kind, StateMutationKind::MachineOwned)
        });
    !writes_machine_owned_field
}

use crate::EmissionPlanningInput;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_state_calls::{StateCallLowering, StateCallRole};
use omega_state_storage::StateMutationKind;

use super::semantic_scope::{proof_scope_suffix, state_name};
use super::{EmissionBlocker, blocker};

/// Guards against the shared-result-slot silent miscompile.
///
/// A computing value-machine — one that returns a `let`-bound local — called two or more times
/// in a single state, with each result stored STRAIGHT TO A FIELD, makes every call site read
/// the LAST call's result. The cause: the callee's internal `let` slot is allocated once per
/// caller state and reused across the inlined call sites, and a field-store defers its result
/// capture to after the call bodies have run, so all the captures read the final slot value.
///
/// The manifesting condition is the CALLER's capture, not the callee's purity: a result consumed
/// in an expression (`let x = f(); let y = f(); out = x + y`) is materialized eagerly and is SAFE
/// (this is why `stack_vm`'s `let b = pop_val(); let a = pop_val();` works), whereas a result
/// stored directly to a field (`self.a = f(); self.b = f()`) is deferred and reads the shared
/// slot. So the gate is: two or more value-calls to the same let-returning callee whose results
/// are each written to a machine-owned field.
///
/// Until per-call-site slots exist, native emission would silently miscompile this shape, so we
/// turn it into a clean error. The sound workaround is one such value-call per state, or binding
/// the results to locals and consuming them in an expression rather than storing each to a field.
pub(super) fn collect_shared_value_call_slot_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    // Field-store value-calls to a let-returning callee: (source, target, stmt, ordinal).
    let mut sites: Vec<(StateKey, StateKey, usize, usize)> = Vec::new();
    for (_, call) in input.state_calls.calls.iter() {
        if !call.required || !call.reachable || !call.target_key.is_valid() {
            continue;
        }
        // Only value-position calls capture a result; a plain `Statement` call is control flow.
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
        // Direct-return callees (`-> param`, no local) have no shared slot.
        if !callee_returns_a_local(input, call.target_key) {
            continue;
        }
        // Only a result stored straight to a field is deferred; a result bound to a local and
        // consumed in an expression is materialized eagerly and is safe.
        if !call_result_stored_to_field(input, call.source_key, call.statement_index) {
            continue;
        }
        sites.push((
            call.source_key,
            call.target_key,
            call.statement_index,
            call.call_ordinal,
        ));
    }

    // Flag any (source state, callee) pair reached by two or more distinct field-store call sites.
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
                "{} stores {} value-calls to the computing machine {} straight to fields in one \
                 state; native emission would share the callee's result slot across the call \
                 sites, so every field reads the last result. Use one such value-call per state, \
                 or bind the results to locals and combine them in one expression{}",
                state_name(input, source),
                distinct.len(),
                state_name(input, target),
                proof_scope_suffix(input, source),
            ),
        ));
    }
}

/// The callee has an internal `let` local (the slot that is shared). Direct-return machines
/// (`-> param`) have none, so they are never at risk.
fn callee_returns_a_local(input: &EmissionPlanningInput<'_>, target: StateKey) -> bool {
    input
        .state_storage
        .locals
        .iter()
        .any(|(_, local)| local.source_key.machine == target.machine)
}

/// The statement that made this call writes a machine-owned field — the deferred capture that
/// exposes the shared slot. A call whose result initializes a local (a `let`) is materialized
/// eagerly and does not match.
fn call_result_stored_to_field(
    input: &EmissionPlanningInput<'_>,
    source: StateKey,
    statement_index: usize,
) -> bool {
    input.state_storage.mutations.iter().any(|(_, mutation)| {
        mutation.source_key.machine == source.machine
            && mutation.source_key.state == source.state
            && mutation.statement_index == statement_index
            && matches!(mutation.mutation_kind, StateMutationKind::MachineOwned)
    })
}

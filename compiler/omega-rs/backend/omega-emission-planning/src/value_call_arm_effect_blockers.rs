use crate::EmissionPlanningInput;
use crate::blocker;
use crate::semantic_scope::state_name;
use omega_backend_report_types::EmissionBlocker;
use omega_state_calls::StateCallRole;
use omega_state_storage::StateMutationKind;
use psi_arena::Arena;

/// An inline VALUE-position call whose callee has an EFFECTFUL arm state is a
/// silent miscompile: the arm-body straight-line expansions are emitted
/// UNGUARDED (every arm's body runs, not just the selected one) and the
/// caller's own statement op re-emits them (the known duplication). Probed
/// 2026-07-07: `self.vec = self.delta(d)` where each arm bumps a counter
/// delivered the CORRECT struct (leaf captures are guard-paired) but
/// hits = 22 -- both arms ran, twice each (11 x 2). PURE arm bodies (`let`
/// decode locals -- the fs wrapper pattern) are unaffected: wrong-arm locals
/// compute into their own unused slots and duplication of a pure compute is
/// invisible. So reject exactly the effectful case: a MachineOwned mutation
/// or a host call in a NON-ENTRY state of a value-called machine. The
/// callee's ENTRY body stays allowed (entry effects are ordered by the
/// deferral machinery and pinned by canaries).
///
/// 2026-07-07b: `&mut` PARAM mutations in arm states (ParameterOrAlias kind)
/// are fenced too -- probed `tally.count + 1` in one arm of a two-arm value
/// callee and read back 11 (BOTH arms ran; the param face skips the
/// re-emission doubling but not the all-arms execution). The tripwire
/// canaries' param-mutating callees keep their effects in ENTRY bodies and
/// stay accepted.
pub(crate) fn collect_value_call_arm_effect_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, state_call) in input.state_calls.calls.iter() {
        if !state_call.reachable {
            continue;
        }
        if !matches!(
            state_call.role,
            StateCallRole::AssignmentValue
                | StateCallRole::CallArgument
                | StateCallRole::TransitionArgument
                | StateCallRole::TransitionGuard
        ) {
            continue;
        }
        let callee_machine = state_call.target_key.machine;
        if !callee_machine.is_valid() {
            continue;
        }
        // A call that ROUTED TO DISPATCH is exempt (re-added 2026-07-09f,
        // SOUND this time): dispatched arms are real dispatch cases running
        // once, and -- the piece the retracted first attempt lacked -- every
        // dispatched terminal now either has a SELECTED return-write or
        // refuses loudly (collect_call_result_return_blockers), so a
        // dispatched value call can never silently ZII. Runtime differential
        // proof: calls/runtime_dispatched_effectful_reentrant_exit (the
        // retraction counterexample, now 70/70) + the seven pinned
        // return-write shape canaries.
        if crate::dispatch_route::state_call_routed_to_dispatch(input, state_call) {
            continue;
        }
        let entry_state = state_call.target_key.state;

        let effectful_arm = input
            .state_storage
            .mutations
            .iter()
            .find(|(_, mutation)| {
                mutation.source_key.machine == callee_machine
                    && mutation.source_key.state != entry_state
                    && matches!(
                        mutation.mutation_kind,
                        StateMutationKind::MachineOwned | StateMutationKind::ParameterOrAlias
                    )
            })
            .map(|(_, mutation)| mutation.source_key)
            .or_else(|| {
                input
                    .host_calls
                    .calls
                    .iter()
                    .find(|(_, host_call)| {
                        host_call.source_key.machine == callee_machine
                            && host_call.source_key.state != entry_state
                    })
                    .map(|(_, host_call)| host_call.source_key)
            });

        let Some(effect_key) = effectful_arm else {
            continue;
        };

        blockers.insert(blocker(
            "state calls",
            &format!(
                "{} statement {}: the value call's callee has a SIDE-EFFECTING arm \
                 state ({}) -- inline value-call arm bodies currently run for EVERY \
                 arm and re-emit, so the mutation/host call would fire multiple \
                 times instead of once. Move the side effect into the callee's \
                 ENTRY body (before its transition), or call the machine as a \
                 statement and read the result from a field.",
                state_name(input, state_call.source_key),
                state_call.statement_index,
                state_name(input, effect_key),
            ),
        ));
    }
}

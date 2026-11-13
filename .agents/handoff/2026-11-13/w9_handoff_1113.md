# worker-9 handoff, 2026-11-13

## Done
- Lane "Q8 caller-inferred helper facts (5 signed `shift_left` rows)" is met on main by #2680, #2708
  and #2785. No code of mine landed for it.
- PR #2795 deleted the clause from TASKS.md; merged as `2efb04d87`.

## Measured
Harness: `target/debug/examples/route_one_comparison <fixture>/main.omg` (prints `program ... route=`).
- At `380d1c537` (before #2785): all five
  `pass/facts/signed_shift_left_bound_through_{call_result,local,other_machine,other_state,parameter}_exit`
  were `route=refused-module`, `ForeignIdentity: a planned private arrival is not a call operation`.
  Cause: `entry_inputs.rs` `require` updated only `OperationKind::Call`; the helpers return nothing
  (`CallUnit`). The route decided nothing, so the old checker's verdict stood.
- At `d351e60be` (#2785): all five `route=accepted`, `unproved=0`. Negative twins `call_result` and
  `local` are `route=rejected`, `fragment=all` for ``may overflow `i64` ``. The other three negative
  twins were not re-measured after #2785 (they were rejected with the same fragment on my fix at
  the earlier base, corpus gate, 20 fixtures, 0 regressed).

## Left over, not landed
- `w9_superseded_fix.patch` (this scratchpad): my version of the same fix, written before #2785
  landed. Two parts main lacks: (1) `OperationKind::module_invocation_requirements_mut` in
  `terminal-psi/src/control_flow.rs`, which also covers `CallStructural` (main's match in
  `entry_inputs.rs` names four forms and omits it); (2) two unit tests for a private helper that
  returns nothing. Not applied or tested against current main. Low value; apply only if a private
  helper called through `CallStructural` shows the same `ForeignIdentity` refusal.

## Observations
- In the negative twins the route no longer reports `shift_left`'s own `val << 2`: `shift_left`
  has no `requires`, is not `pub`, and gets its input inferred from its one caller. The rejection
  now comes from `done`'s `s - 100`. The pinned fragment still matches.
- `omega --check <fixture>` on this Windows host (default target) failed for all five with
  `selected completion provider NativeConsole::exit_process: completion call has no original
  external satisfaction leaf` at `380d1c537`. The corpus gate checks `macos_arm64` and passed. Not
  investigated.

## Next step for the next lane ("the rest of generics")
Not started. The refusal to chase is `machine generic parameters`
(`structure-elaboration/src/read/machine.rs` `machine_shape`, lines ~438-475): lifetime
parameters, linear type parameters, callable and runtime value binders. The 2026-11-12 coverage map
(PR #2745, pin `0835b3aaf`) lists 95 `pass/generics` and 2 `run/generics` fixtures under it; that
list predates #2674 and must be re-measured. PR #2777 (open draft) edits `lowered-discharge` and
`structure-elaboration/src/{lib.rs,read/machine.rs}`; coordinate before touching those files.

## Slot
`C:\Projects\Omega\.agents\worktree\worker-9`: detached at `2efb04d87` (main), clean, nothing running.
`target/debug/examples/route_one_comparison.exe` is built at `d351e60be`; `omega.exe` is at `380d1c537`.

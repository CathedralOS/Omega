# Completion cleanup through a ranked cycle

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../TASKS.md).
This is another production of the existing joint-rank fact:

A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

`perform` moves an owned `Resource` into the nested `count`/`forward` component.
The resource holds the caller's `&mut Counter`, so each cycle edge advances
`counter.steps` through the moved carrier, and the resource's `drop` hook adds
to `counter.drops`. The cleanup must run exactly once, when the component
completes in `done`, never on an intermediate edge and not twice.

The matching input performs seven steps (3 to 10) and one drop. The fallback
input completes at once: zero steps, one more drop. Both run in every `main`.
A lowering that dropped on each edge would read 8 drops after the first call
and one that never dropped would read 0; neither survives the exit-0 oracle.
The `canary` field beside the counters stays 641.

All nine places contain `completion_cleanup/main.omg` and its `invalid/main.omg`
twin: local, field, parameter, call result, match join, const name, nested
machine, another machine and selected implementation. Only the way `remaining`
reaches `count` differs. The selected-implementation place is the ordinary
`HostedCleanupBody`, selected by Build for `HostedCleanup`. Every positive binds
all four hosted `ProgramEntry` targets and requires native/Terminal exit 0.
Every twin changes only `pending - 1` to `pending` and must reject strict
descent. The exact programs are pinned as
`tests/omega/{pass,fail}/facts/nested_machine_joint_rank_completion_cleanup_*`.

Three controls extend the row:
[a 100,000-edge cycle](../../controls/completion_cleanup_long_cycle/main.omg)
keeps the borrowed counter live throughout and drops once at the end;
[the missing precondition](../../controls/completion_cleanup_missing_precondition/main.omg)
removes `requires resource.valid == 1u64` from the entry `count` and rejects at
its first call, because the carried drop precondition no longer holds there;
[a live local loan](../../controls/completion_cleanup_live_local_loan/main.omg)
builds a droppable `temporary` over the cycle's counter inside `forward` and
rejects the terminal call that passes the counter on, as at any call, since the
pending drop still uses that loan.
In scratch probes (not pinned) the nested helpers could omit the precondition
clause, whose requirement is then inferred from what they call; the entry could
not.

## Evidence

Fixed base `0ff9f81b3933bb16aaa6eddd05fc7a40107252b0`. Producer: this source
built with `CARGO_PROFILE_DEV_OPT_LEVEL=1` (`omega --build-profile` reports
`opt_level` 1). Host: Windows x86-64 only; macOS ARM64 and Linux were not run.

- All nine positives: `omega run --both` native exit 0, Terminal Psi exit 0.
- All nine twins: rejected with `a termination promise additionally requires
  strict descent on every complete cycle`.
- Long-cycle control: native and Terminal exit 0 with 100,003 steps and one
  drop per invocation. The emitted PE image reserves 1 MiB of stack. A copy with
  the reserve patched to 64 KiB also exits 0; 100,000 frames could not fit in
  either. Smaller reserves may be rounded up by the loader, so no smaller bound
  is claimed.
- Missing-precondition control: rejected with `cannot prove requires contract
  for call forward from count`.
- Live-local-loan control: rejected with `state count receives counter while
  local borrow temporary is still active`.

## Requirements

- Establishment: construction sets `valid: 1u64`; `count` authors the drop
  precondition and every edge into `forward` and `done` is checked against it.
  The recursive contract does not establish itself and `pending > 0` is proved
  before the exact subtraction.
- Identity and lifetime: one resource and one borrow of the caller's counter
  span every edge. The caller reads `canary`, `drops` and `steps` after
  completion and finds them in the original storage.
- Composition: ordinary terminal calls pass the owned resource with exact
  actuals; no enclosing runtime value is captured. Each place keeps the same
  operation.
- Soundness: ranking, the drop precondition and borrow ownership remain
  independent obligations. Ranking does not supply the precondition.
- Psi 06-08: the drop is the automatic scope-exit cleanup of the moved
  resource at `done`. Terminal Psi and native execution agree on one drop per
  invocation and on the unchanged canary.
- Omega 00-09: ordinary calling plans and register/frame realization carry the
  moved resource; the constant native stack on the long cycle is the evidence
  above. This is not a portable composed WCSU certificate.

Open: composed WCSU, other-host physical limits, and the rest of the parent
item.

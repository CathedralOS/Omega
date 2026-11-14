# Suspending terminal cycles

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../TASKS.md).
Requirement: sibling/ancestor terminal cycles with exact call markers, executed
source-to-Terminal/native (`omega run --both`) in constant stack, with the
effects/markers part of the complete callee contract.

> A ranked terminal cycle of nested machines whose every edge acknowledges `suspend` adds each parked step's answer and forwards the total to its ordinary caller.

`sum` is ranked by `remaining`. Its nested `step` parks on
`Scheduler::advance`, then transfers to its sibling `fold`, which transfers back
to the ancestor `sum` with `remaining - 1`. Every edge is a terminal call whose
callee may suspend, so every edge carries `suspend`. The inline scheduler
answers three times its argument. Main runs 100,000 rounds (expected total
15,000,150,000) and an empty run that must return its start value 7, so
dropping or replacing a parked answer, taking the wrong arm or losing the start
value traps.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [source](local/main.omg) | [missing marker](invalid_local/main.omg) |
| Field | [source](field/main.omg) | [missing marker](invalid_field/main.omg) |
| Parameter | [source](parameter/main.omg) | [missing marker](invalid_parameter/main.omg) |
| Call result | [source](call_result/main.omg) | [missing marker](invalid_call_result/main.omg) |
| Match join | [source](match_join/main.omg) | [missing marker](invalid_match_join/main.omg) |
| Const name | [source](const_name/main.omg) | [missing marker](invalid_const_name/main.omg) |
| Nested machine | [source](other_state/main.omg) | [missing marker](invalid_other_state/main.omg) |
| Another machine | [source](other_machine/main.omg) | [missing marker](invalid_other_machine/main.omg) |
| Target body | [source](target_body/main.omg) | [missing marker](invalid_target_body/main.omg) |

Each twin removes only the `suspend` from the `fold -> sum` edge and rejects
with ``call to `sum` has operational envelope `suspend` but acknowledges
neither suspension nor blocking``. The field place checks canaries on both
sides of the round count; the target-body place selects `X86Cycle` or
`ArmCycle` through Build, and each adds its own start tag (17 or 29).

Two controls exercise the same fact: [a guarded completing arm that is itself
a suspending boundary call](controls/guarded_boundary_arm/main.omg) (empty path
answers 21), and
[the caller's exclusive borrow carried across every parked step](controls/borrowed_backing/main.omg)
(100,000 increments, canaries intact).

## Nearest rows

[Sibling](../sibling_cycle/main.omg) and [ancestor](../ancestor_cycle/main.omg)
cycles and [joint ranks](../joint_rank/README.md) run the same terminal-cycle
shape without suspension; their twins name strict descent. These twins name a
missing call acknowledgement, and the positives exercise a different layer: the
Psi 06 join of checked suspension crossings to Terminal operations.

## Baseline and repair

At base `4f87c9e08b1` all nine positives and both controls checked and then
refused Terminal production with
`Lowering(Unsupported("possibly-suspending checked call has no exact Terminal operation join"))`;
the guarded-arm control refused with
`Lowering(Unsupported("suspension frontier value is unavailable at its call"))`.
All nine twins already rejected with the marker diagnostic. The same refusal
hit any suspending runtime cycle: a self cycle, a top-level pair, an unranked
productive cycle.

A terminal transfer between states of one composed recursive component lowers
to a successor edge, not a call operation, so its checked crossing had no
operation to join. The repair keeps that crossing (and its carry checks) and
publishes no call plan for it when stage 05's composed plan names a successor
at that coordinate entering the target and every Terminal machine holding the
source state also holds the target's entry state; the component's real parking points
keep their own plans. A frontier value is also visible from a block that
dominates the call, as a composed state's header parameters do for a guarded
arm's block.

## Requirements

- Establishment and composition: the rank comes from `terminates by remaining`;
  `step` and `fold` preserve it and `fold` decreases it, and every arrival
  proves `remaining > 0` from its guard. Exact markers come from each callee's
  envelope; nesting grants no exemption.
- Identity and lifetime: scalar actuals are evaluated once; the borrowed-backing
  control keeps the caller's exclusive loan live across every parked step and
  observes it after return.
- Psi 06–08: crossings retain their checked live values; fused edges add no
  activation; parking calls keep their suspension call plans. Terminal Psi
  execution returns exit 0 for every positive.
- Omega 00–09: each positive binds the four hosted `ProgramEntry` targets plus
  `macos_x86_64` for this host. Native execution was observed on macOS x86-64
  only. A kept native image of the local place runs 1,000,000 rounds under a
  64 KiB `RLIMIT_STACK`, the same floor as an empty program on this host
  (at 32 KiB both the empty program and the 100,000-round image fail at
  startup), so the cycle adds no stack per round.

Pins: `tests/omega/{pass,fail}/facts/nested_machine_suspending_cycle_*`.

## Parking scheduler

The inline scheduler answers at once, so `omega run --both` never parks. The
Terminal-interpreter test
`readiness_parks_every_step_of_a_fused_suspending_cycle`
(`omega-rust/omega/compilation/compiler/tests/terminal_effect_readiness.rs`)
runs the unchanged local place with `Scheduler::advance` bound to a host
scheduler that reports "not ready" twice for every call before answering three
times its argument. The activation parks 200,000 times (twice per round), a
re-poll charges no work, arguments arrive as 100,000 down to 1, and the
program's own oracle completes with Unit, as with the inline scheduler.

At base `11bdd2fe41f` the first park failed with ``host readiness cannot
suspend a call without a checked suspension crossing``. Stage 06 bound
crossings only for source machines, and the fused component's members `step`
and `fold` lower into `sum`'s composed machine, so the `Scheduler::advance`
call in `step` carried no crossing and no suspension plan. The inline
scheduler never asks to park, so every place still exited 0. Crossings are now
bound for every emitted state; the member-to-member transfers are then
recognised as fused edges, as before.

## Open

Native execution on the four hosted targets is not observed here. Native
realization has no task runtime that parks and resumes an activation, so a
natively parking scheduler waits on **TR3-TR8**; native runs here use the
inline scheduler. The parent outcome keeps its remaining invocation/custody
acceptance, product-compiler support and migration.

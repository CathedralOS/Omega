# Whole-compile profile of the resolution boundary, 2026-10-24

The three remaining symbol-resolved tree copies, the preparation syntax-forest
clone, and the early call-result pass are not the compile. On these three
workloads they cost about 2 ms inside a 1.1–1.4 s compile. Checking does.
Nothing in the production compiler changed.

Source base: `9798e8e31a3dfb4fcdad825aaa69d6ed0a5d38a6`. The measured binary was
that source plus the temporary probe in [instrumentation.patch](instrumentation.patch).
The probe was removed after capture. Binary SHA-256:
`a3fd536015acdf635f86b1e5f1447f549206c748d4ac4c5a93caa8216a648d57`.
Dev profile, effective opt-level 1. Host: Windows 11, AMD64 Family 25 Model 97
Stepping 2, 32 logical processors. One warmup and four measured compiles per
workload. Every measured image that compiled executed with native exit 0.

`OMEGA_RESOLUTION_MEASURE` timed the boundary probes. `OMEGA_PHASE_PROFILE`
timed the existing stage 05 scopes. Clone timers exclude the result print.
Parent scopes and wall time include it. Stage rows overlap: Stage 05 sits
inside the `compile` row, and the phase rows are inclusive of their children.
Do not add those rows.

| Workload | Compiler elapsed | Stage 05 | Stage 03 | Syntax clone | Resolved clones | Call-result selection |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| length | 1,139 ms | 919 ms | 15 ms | 0.11 ms, 72 rows | 1.89 ms, 3 clones (541 / 5,052 / 5,052) | not entered |
| dynamic dispatch | 1,417 ms | 1,146 ms | 17 ms | 0.11 ms, 58 rows | 1.81 ms, 3 clones (539 / 5,050 / 5,050) | not entered |
| nested record | 1,304 ms | 1,045 ms | 18 ms | 0.13 ms, 98 rows | 2.07 ms, 3 clones (558 / 5,069 / 5,069) | not entered |

`retain_applied_results` returns in about 0.01 ms on all three. The guard sees
no call with static machine arguments, so `resolve_call_result_selection` never
runs. This does not time that pass on a placed-view program. Preparation as a
whole, including pre-resolution and both body-completion batches, is 42–49 ms.

Inside Stage 05, the large named scopes on the dynamic-dispatch median are
flow-fact construction (`05.d.3.b.flow_fixed_point.build_flow_facts`, 342 ms)
and standard-library machine checking (`05.d.2.a.2.machine.std_or_core`, 228 ms).
Those are inclusive of their own children and sit under `05.d.lower_typed_trees`
(970 ms). Length and the nested record show the same shape, at 296 / 221 ms and
315 / 223 ms.

Dynamic dispatch and the nested record were compiled in place. Length's
`local/omega.lock` is version 4 and this compiler refuses it, so length was
compiled from a copy of `main.omg` and `build.omg` with that lock left behind.
An earlier copy of `main.omg` alone failed with `terminal-artifact production
requires one exact selected program entry`; that run is not a compile-time
measurement.

Evidence: [in-place runs](observations-inplace.json),
[length runs](length-observations.json), [median reduction](summarize_profile.py),
[length command](length_profile.py).

## Next question

Whether flow-fact construction repeats work that checking already has. That
scope is the largest named piece of the second that these compiles spend.
Another cut of the three resolved copies will not move the compile.

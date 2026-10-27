# Joint private runtime ranks

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../TASKS.md).

Fact: a private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends while advancing the explicit counter once.
Each place transports the same natural input into the same `count` operation.
The caller retains the mutable counter through the cycle and reads it again
after completion. Helpers author their actual entry requirements, without
redundant `terminates by` clauses or captured runtime inputs.

The matching input performs seven increments from 3 to 10. The fallback input
performs zero increments and preserves 10. Both run in every `main`: inverting
a dispatch, selecting the fallback, omitting an increment, or repeating one
fails the exit-0 oracle. Every twin changes only `pending - 1` to `pending` and
must reject because the complete cycle then has no strict edge.

| Place | Positive | Nondecreasing twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [source](local/invalid/main.omg) |
| field | [source](field/main.omg) | [source](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) |
| call_result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) |
| match_join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) |
| const_name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) |
| other_state | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) |
| other_machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) |
| target_body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) |

[Earlier decrease](arrival_decrease/main.omg) moves the strict edge before two
preserving helper calls, with renamed and reordered parameters; its
[twin](arrival_decrease/invalid/main.omg) removes that edge. The
[top-level peer](top_level/main.omg) checks the same rank correspondence without
lexical containment. A [private promise](private_promise/main.omg) declares
`terminates` on its helper without a redundant helper rank; its
[twin](private_promise/invalid/main.omg) still rejects a preserving cycle. [Long cycle](long_cycle/main.omg) performs 100,000 increments
while retaining the caller's original borrowed counter.

Additional source controls reject a [preserving subcycle](controls/preserving_subcycle/main.omg)
even when another cycle decreases, a [non-tail recurrent call](controls/non_tail/main.omg),
a [false arrival requirement](controls/missing_arrival_contract/main.omg), and
[overlapping mutation through a live shared loan](controls/live_shared_loan/main.omg).
The last two are independent contract and ownership obligations: ranking does
not authorize them.

## Evidence and requirements

The fixed fetched base is `56b550d41aec59c4ac3e59ca3d570f1a2afb9dc5`.
It initially fails Rust compilation with E0499 in
`discardable_structural_result_by_ordinal_mut`. The necessary disjoint-field
borrowing prerequisite is `aa4ebe6e319d61c060e969f51646249837a64c21`;
its independently built optimized producer rejects all nine unchanged positives
with `a member lacks a supported exact ranking witness` for `count, forward`.
Raw baseline, candidate, qualification and resource observations are retained in
this workbench's `build/joint-rank/` and identified in the publication record.

- Establishment: the authored natural rank anchors the component. Each private
  coordinate is an exact declaration parameter; every recurrent call is checked
  against its actual arguments and its own current prefix. The source rank is
  its entry snapshot, not a later mutable value or an enclosing entry fact.
- Identity and lifetime: simultaneous argument identity and the explicit mutable
  counter survive preserving and decreasing edges; the caller observes the
  original backing after completion. The live-shared-loan control still rejects.
- Composition: helpers can rename/reorder the rank; every incoming and outgoing
  recurrent occurrence participates. The graph containing only non-strict edges
  must be acyclic, so a convenient decreasing cycle cannot hide a preserving one.
- Soundness: formation, callee requirements, effects, cleanup, loans and exact
  tail-position checks remain separate. Unpromised productive components receive
  no fabricated natural rank. Other views and authored ranges retain their
  existing full-coordinate and endpoint obligations.
- Psi 05–07: admitted coordinates become the existing entry observation and
  constant-stack component rows, including witness-free helpers. Native and
  canonical Terminal execution agree; the pinned corpus independently audits
  emitted operations and custody.
- Omega 00–02: each positive binds all four hosted `ProgramEntry` targets. The
  ordinary caller and explicit borrowed argument use existing calling plans;
  only macOS ARM64 has native execution evidence here.
- Omega 03–06: the long native image exits 0 with an actual 256 KiB
  `RLIMIT_STACK`, with the caller-owned counter live for all 100,000 cycles.
  This bounds this physical execution; it is not portable WCSU certification.
- Omega 07–09: emitted macOS ARM64 images execute the observable counter oracle
  and agree with Terminal Psi. Other hosted native engines remain unmeasured.

The positives and fact-sensitive twins are pinned under
`tests/omega/{pass,fail}/facts/nested_machine_joint_rank_*`.
Remaining parent acceptance includes private coordinates in ranged/structural
views, portable composed WCSU and other-host physical limits, complete effectful
call-contract/custody composition, source migration and the Omega-written
compiler. This row does not close `NESTED-MACHINE-UNIFICATION`.

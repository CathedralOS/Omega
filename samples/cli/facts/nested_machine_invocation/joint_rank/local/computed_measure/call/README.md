# Calls in declared rank measures

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../TASKS.md).

Fact: a private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends while advancing the explicit counter once.
Each place uses the same `Remaining::Complement` measure, ordinary `rank` call,
`0..=18446744073709551615` range and counter operation. Both indexed selectors,
active inputs (4→7) and complete inputs (7→7) execute. The caller checks its
original counter, selected element, untouched neighbor and canary641. Taking
the fallback arm on an active input, skipping or repeating an increment, or
writing the wrong slot traps. Each twin removes only the strict increment.

| Place | Positive | Nondecreasing twin |
| --- | --- | --- |
| local | [source](../../../local/computed_measure/call/main.omg) | [source](../../../local/computed_measure/call/invalid/main.omg) |
| field | [source](../../../field/computed_measure/call/main.omg) | [source](../../../field/computed_measure/call/invalid/main.omg) |
| parameter | [source](../../../parameter/computed_measure/call/main.omg) | [source](../../../parameter/computed_measure/call/invalid/main.omg) |
| call_result | [source](../../../call_result/computed_measure/call/main.omg) | [source](../../../call_result/computed_measure/call/invalid/main.omg) |
| match_join | [source](../../../match_join/computed_measure/call/main.omg) | [source](../../../match_join/computed_measure/call/invalid/main.omg) |
| const_name | [source](../../../const_name/computed_measure/call/main.omg) | [source](../../../const_name/computed_measure/call/invalid/main.omg) |
| other_state | [source](../../../other_state/computed_measure/call/main.omg) | [source](../../../other_state/computed_measure/call/invalid/main.omg) |
| other_machine | [source](../../../other_machine/computed_measure/call/main.omg) | [source](../../../other_machine/computed_measure/call/invalid/main.omg) |
| target_body | [source](../../../target_body/computed_measure/call/main.omg) | [source](../../../target_body/computed_measure/call/invalid/main.omg) |

The [frozen probe](../../../parameter/computed_measure/call/probe/main.omg)
preserves the original smaller source used to locate the refusal. The
[composition probe](../../../parameter/computed_measure/call/probe/composition/main.omg)
computes `rank(identity(value) + 0)`, retaining the same fact and oracle.

- Establishment: each call's complete actuals and public entry requirements
  form before its public result guarantee can contribute facts. Independent
  denotational admission still requires progress and a pure, crash-free closure.
  The private helper body is checked separately; the caller does not unfold it.
- Identity and lifetime: the input is the exact captured indexed element under
  its checked bounds. An opaque call term retains selected target and complete
  operand identities; later operand equalities participate in congruence.
  Entry and arrival observations have distinct invocation identities.
- Composition: the complete declaration application carries its call receipt
  and selected public-contract identity into the ordinary scalar computation
  graph. Both entry and successor observations consume that retained graph.
- Soundness: controls reject [missing entry premises](../../../controls/declared_call_precondition/main.omg),
  [self-establishing premises](../../../controls/declared_call_self_establishment/main.omg),
  a [published crash ceiling](../../../controls/declared_call_crash_ceiling/main.omg)
  and an [unproved progress dependency](../../../controls/declared_call_nonterminating/main.omg).
  `ensures` alone proves no return. Existing ownership and tail-cycle rules remain
  independent; declaration calls do not manufacture statement loan captures.
- Psi 05–07: declaration calls enter the shared call scheduler and emitter.
  Straight-line direct calls produce ordinary operations without artificial
  control edges; selective control retains the block route and rank-dominance
  check. No new measure interpreter, body folding or Terminal operation exists.
- Omega 00–09: ordinary calling plans, storage, instruction lowering and image
  emission execute the counter oracle. All positive programs bind all four
  hosted targets; actual native execution here covers macOS ARM64 only.

The fixed original base is `2feafe36a0b7de29a61055e003996dd7574ea8c1`, with the
explicit header-delimiter prerequisite. The isolated optimized C6 baseline
refuses all nine positives at rank-range/joint-witness checking. Optimized core6
(`opt-level=1`) executes nine positives and both probes with native exit0 and
Terminal Psi exit0; all nine twins reject strict non-descent. Raw source hashes,
producer bytes, commands, streams and timings are retained in this workbench's
`build/call-rank/` and bound by the final qualification/publication record.
Historical core1–5 refusals remain labelled with their actual producer identities.

Corpus pins are `tests/omega/{pass,fail}/facts/nested_machine_joint_rank_declared_call_*`.
This establishes scalar call-computed rank observations, not all structured,
cross-carrier or nonscalar measures, composed WCSU acceptance, other-host native
execution, complete language migration or the Omega-written compiler. Those
remain on the parent task.

# Wrapping index arithmetic

Every recorded offset is below 495, so its wrapping sum with 17 is below 512
and `read` can index the window. This discharges a numeric-observation and
qualified-storage requirement of `NESTED-MACHINE-UNIFICATION` in
[TASKS.md](../../../../../../TASKS.md), exposed by directory-buffer reads.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [weakened bound](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [weakened bound](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [weakened bound](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [weakened bound](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [weakened bound](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [weakened bound](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [weakened bound](other_state/invalid/main.omg) |
| Another machine | [program](other_machine/main.omg) | [weakened bound](other_machine/invalid/main.omg) |
| Selected implementation | [program](target_body/main.omg) | [weakened bound](target_body/invalid/main.omg) |

Optimized baseline `1e9d4d0939bd60731383da4630bc0150dd1d7b4c` refuses
all eighteen at the intended array index. A direct offset copy and equivalent
Exact addition already check; a Wrapping temporary also refuses. Sources and
oracles were frozen before compiler edits. Repaired producer
`14f266609aa5729eee533c52726a754d5918a86d` runs all nine positives with
native/Terminal exit 0 on macOS ARM64; all nine twins still reject the index.
The source tree/qualification records retain the actual full producer identity.
The same programs are pinned under
`tests/omega/{pass,fail}/facts/nested_machine_wrapping_index_*`.

Captured finite arithmetic uses the existing policy engine after independently
formed premises are live. Its original opaque value remains usable if exploring
the computation declines. Exact children must form before the exploration can
supply an equation. Actual wrapping reduction and saturation clamping happen
before a result participates in integer laws. Interval reduction must preserve
the entire interval width before endpoints establish a narrow wrapping range.

Structural path discovery separately retains the qualified endpoint while
inspecting reference shells. Previously it returned bare `u64`, making stage 04
record Exact addition for Wrapping/Saturating fields. The controls exposed an
actual Terminal failure and a native saturation trap; retaining the endpoint
fixes their producing path without changing interpreter admission.

- **Establishment:** each consuming edge has the same `offset < 495` fact;
  its unsigned sum with 17 is at most 511. The twins change only that bound
  to 496, permitting index 512. No helper precondition or caller equality guard
  supplies the result.
- **Identity and lifetime:** the bound and calculation use the exact current
  field. An [overwritten offset](controls/overwritten_offset/main.omg) rejects;
  completed values and their argument/reference origins remain distinct from
  later storage versions.
- **Composition:** local, field, explicit parameter, call-result, match, const,
  nested and ordinary-machine transports retain the same bound. The ninth
  place uses an ordinary named implementation selected by Build. A
  [nested policy calculation](controls/nested_policy/main.omg),
  [product](controls/multiply/main.omg),
  [difference](controls/subtract/main.omg), and
  [bounded saturation](controls/saturating_no_overflow/main.omg) also run in
  both engines using the same finite-value projection owner.
- **Soundness boundaries:** guards on [actual wrapped values](controls/actual_wrap/main.omg)
  and [actual clamped values](controls/actual_clamp/main.omg) remain usable.
  A [wrapped result cannot bound its original input](controls/wrapped_guard_original_offset/main.omg).
  [Crossing a wrap boundary](controls/crossing_band/main.omg),
  [spanning modular bands](controls/wide_interval/main.omg), and an
  [unsafe Exact child](controls/exact_child_formation/main.omg) reject. Goals
  never enter the premise worklist; no theorem induction or denotation is added.
- **Checked-to-Terminal Psi:** qualified field observations retain their
  selected arithmetic policy. Real wrap and clamp controls execute their
  overflow cases and distinguish the stored results; they no longer emit
  Exact addition. Each row program checks the upper valid index, zero offset,
  rejected boundary, maximum input, disabled edge, saved indices and canary.
- **Provider settlement, layout and calling plans:** every positive binds all
  four hosted entries. macOS ARM64 execution exercises explicit mutable
  reference calls, the 512-byte inline array and its sibling scalar fields.
  Other-host native execution remains unverified.
- **Instructions and activation storage:** native reads must use the correct
  selected policy, field offsets and caller-owned storage; saved scalar results
  must survive subsequent calls. This row adds no new long-cycle stack claim.
- **Encoding and image emission:** direct native images execute and agree with
  Terminal Psi for all nine places and six positive controls. Native saturation
  must clamp rather than reduce; Terminal must execute a wrapping operation
  rather than a misclassified Exact instruction. The unchanged reject twins
  remain checking obligations.

The actual bundled-library replay removes all seven directory-buffer index
diagnostics. On this fixed library source, five Windows completion guarantees
and two unclassified tail cycles remain. Focused regression observations also
retain fourteen source failures and twelve native refusals that reproduce
identically with the pre-repair producer; their goldens remain unchanged.

This row does not close the parent task. Full bundled-library admission,
remaining call/completion contracts, broader cyclic measures and the
Omega-written product/source declaration migration remain open. Focused
regression/qualification records and the claim retain producer, source and
host identities; they do not assert cross-host runtime acceptance.

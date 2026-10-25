# Codec writes through a stored reference

Encoding through an owned reference field preserves a disjoint index bound.

This advances **R5** in `TASKS.md`: caller-visible may-write summaries for an exclusive codec cursor stored in an owned carrier. `Packet::encode` writes `4` through `Counter.slot` while encoding scalar `200`. A separate index starts at `1`; indexing `[4, 7, 8, 9]` must still read `7`. Each invalid twin changes only the cursor referent to the index storage. The overlapping write must retire the bound.

The root and nine places bind all four hosted `ProgramEntry` targets and require exit `0`. The available executor is Linux x86-64. Other target bindings establish source coverage, not foreign-host execution.

| Place | Positive | Invalid twin |
| --- | --- | --- |
| local | [local](local/main.omg) | [overlap](local/invalid/main.omg) |
| field | [field](field/main.omg) | [overlap](field/invalid/main.omg) |
| parameter | [parameter](parameter/main.omg) | [overlap](parameter/invalid/main.omg) |
| call result | [call result](call_result/main.omg) | [overlap](call_result/invalid/main.omg) |
| match join | [match join](match_join/main.omg) | [overlap](match_join/invalid/main.omg) |
| const name | [const name](const_name/main.omg) | [overlap](const_name/invalid/main.omg) |
| a nested machine | [state](other_state/main.omg) | [overlap](other_state/invalid/main.omg) |
| another machine | [machine](other_machine/main.omg) | [overlap](other_machine/invalid/main.omg) |
| another target body | [target body](target_body/main.omg) | [overlap](target_body/invalid/main.omg) |

Run `omega run --both samples/cli/facts/may_write_codec/<place>/main.omg` and `omega --check samples/cli/facts/may_write_codec/<place>/invalid/main.omg`. Invalid projects require `cannot prove`. Positive and negative places are pinned under `tests/omega/{pass,fail}/facts/may_write_codec_through_<place>_{exit,overlapping}`.

The governing contracts are [live storage and mutation summaries](../../../../wiki/spec/language/state_contracts.md#mutation-and-subject-identity) and [ordinary checked codec machines](../../../../wiki/spec/layouts/codecs.md). Generation establishes a typed callable; it supplies no borrow authority or storage-origin evidence.

## Requirements

1. Establishment: the independent index is initialized to `1`, or arrives under `other < 4`, before encoding. No post-call equality guard re-establishes its bound.
2. Identity and lifetime: the owned carrier stores the cursor's captured referent. The invalid twin changes that referent to the index; its prior bound is stale.
3. Composition: the preserved bound feeds indexing directly, through a field, a parameter, a result contract, a match join, a named constant, a state arrival, a forwarding machine or a target-scoped body.
4. Soundness: write footprints never authorize loans, moves, linear claims or a reference load behind another reference. Unknown origins stay opaque.
5. Psi 05–07: the ordinary generated codec and carried cursor lower into Terminal Psi and execute with `--both`.
6. Omega 00–02: all hosted entry bindings use the ordinary checked callee, reference placement and selected realization.
7. Omega 03–06: native execution writes the codec cursor through the carrier's reference and reloads the independent index.
8. Omega 07–09 and emission: the native image and Terminal interpreter must both observe exit `0` on the actual host.

This row does not close R5's finite write cycles, parameter permutations or parser convergence. Non-place codec operand type inference, unknown origins and reference loads through borrowed carriers remain outside this repair.

## Observed baseline

On Linux x86-64 at `d6cf554fd1cbf20551983d29bae26f5cf3b81664`, the frozen root rejects `cannot prove index` and several places instead reach the missing codec-callee lowering diagnostic. None of the ten positives executes. Seven overlapping twins incorrectly pass checking. The repaired row must retain all authored sources and require ten native/Terminal exit-0 agreements and nine intended bound rejections.

The extra corpus variations exercise an array of reference carriers, occurrence-local declaration typing across states, and rejection of a shared cursor. The existing codec-choice and signed round-trip witnesses execute with exit 0 on baseline and candidate. The legacy exact-array fixture refuses the same unaccepted package-entry binding on both; no execution is claimed for that fixture.

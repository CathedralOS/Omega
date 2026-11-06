# Checker citation replay, 2026-11-05

One library proof, `rat_close_triangle_split` in `source/library/core/rat.omg`,
costs about 830 ms of every compile that loads the float stack, and 6.7% of all
attributed checker work across a 510-test corpus shard. The cost is not the proof
itself: proving each lemma call's preconditions re-walks the caller's statement
prefix and re-proves every earlier lemma citation in it, so the cost of a call grows
with the number of lemma calls before it.

Attribution only; no optimization was tried. Next experiment is listed at the end.

Host: Windows 11, 32 logical CPUs, shared with other compiler builds. Producer:
optimized development profile (opt-level 1, debug assertions on), main
`b4ff5dfcc3c` plus the per-call-site profile labels in this change. Workload: the
one-line program `data Main { marker: u64; } machine Main::main(main: &mut Main) {
main.marker = 0; }` with `build.omg` from `tests/omega/pass/arithmetic/addr_algebra`,
`OMEGA_PHASE_PROFILE=1 omega --check --timings`. Times are inclusive wall time on
the checking workers, summed over the two checks that load `core/rat.omg`.

## Cost grows with the number of earlier lemma calls

Per-call-site subject rows under `check_flow_call_contracts.calls`, for calls made
by `rat_close_triangle_split`:

| Statement | Callee | Earlier lemma citations | Cost |
| ---: | --- | ---: | ---: |
| 0-23 | `add`, `mul`, `rat_gap` (no preconditions) | - | 0.005-0.03 ms each |
| 26 | `mul_le_mul_right` | 2 | 34 ms |
| 27 | `mul_le_mul_right` | 3 | 39 ms |
| 28 | `mul_le_mul_right` | 4 | 52 ms |
| 29 | `add_le_add` | 5 | 142 ms |
| 30 | `le_trans` | 6 | 182 ms |
| 31 | `mul_le_cancel_right` | 7 | 228 ms |
| 32-36 | `mul_one_right`, `mul_succ_right`, `le_one_succ` (no preconditions) | - | 0.003-0.02 ms each |
| 37 | `mul_le_cancel_right` | 13 | 387 ms |

Calls without preconditions stay near 0.01 ms at any position, so position alone is
not the cost. Calls that must prove preconditions grow 11x from statement 26 to 37.

## Mechanism

For each call requirement, `call_requirements.rs` builds a structural judge and
`structural_case_arms::recognize_structural_state_prefix` walks the caller state from
its first statement to the call site. At every earlier lemma call in that prefix,
`establish_citation` builds a fresh site judge, feeds it all guarantees established so
far, and `instantiate_citation` re-proves that lemma's preconditions before admitting
its guarantees. The call at statement 37 therefore re-proves the preconditions of the
lemma calls at statements 24-31, each against a growing guarantee set, and the whole
walk repeats for every precondition of every checked call.

Phase rows for the same compile agree: `captured_boolean.structural` 1199 ms over 98
requirement checks; inside it `cap.recognize` (the prefix walk) 934 ms, and
`cap.citation` 637 ms over 318 citations.

## Corpus shard

Shard 1/32 (510 tests, six batches of up to 100, 12 compiles in parallel, profiler on):
`rat_close_triangle_split` is the most expensive function, 1302 s of 19496 s attributed;
its call edge to `mul_le_cancel_right` alone is 611 s. Absolute seconds are inflated by
host contention; the ranking is what this records.

## Next experiment

Walk each state prefix once per check and admit each citation once, then answer every
call site in that state from the guarantees established at its position.
`recognize_structural_state_prefix_batched` already shares one walk among visitors at a
single site; the candidate extends that to all sites of a state. Expected: the
`rat_close_triangle_split` call checks fall from about 830 ms to the cost of one walk.
Acceptance: identical status and diagnostics on the corpus shard and on every fixture
under `tests/omega/{pass,fail}/proofs` and `contracts`, plus the one-line program's
per-call-site rows. Rejection: the shared walk cannot preserve each site's diagnostics
or its pruning by site, or the wall-time change on the one-line program is within noise.

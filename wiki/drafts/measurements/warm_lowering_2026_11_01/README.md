# Warm lowering: provider-catalog rediscovery

2026-11-01, Windows x86-64, Ryzen 9 7950X, shared host. Release compilers
(`opt_level` 3) built from main `62ce026b593`: baseline SHA-256
`60247a071d05f9ed…`, candidate = baseline plus this change.
**Decision: retain.** Warm Dungeon compile median 12.07 s -> 9.58 s (-20.6%),
byte-identical native output.

Follows [compile scaling](../compile_scaling_2026_10_31/README.md), which found
that an unchanged warm Dungeon compile spends 75% of its time in lowering and
settlement. The question there was whether to reuse those results across
compiles. Attribution first showed most of that time is avoidable work inside
one compile, which reuse would only hide.

## Attribution (warm Dungeon, `OMEGA_PHASE_PROFILE=1` plus temporary prints)

Lowering runs four times per single-target compile: Build evaluation (7
machines, 9 ms), a Build provider (4 machines, 17 ms), calling-policy
declarations (134 machines, 2.0 s), and the product (222 machines, 6.3-7.4 s
under profiling). The two large ones split as:

| Scope | Calling policy | Product |
| --- | ---: | ---: |
| `05.01.a_discover_call_catalog` | 1.69 s, 3 rounds | 1.93 s, 3 rounds |
| of which `checked_unit_provider_candidates` | 99% of each round | 99% of each round |
| `05.10.c_payload_memberships` | small | 4.4 s (2 machines: 2.97 s + 1.40 s) |

Discovery rounds cost 530-720 ms each whether the closure held 97 or 222
machines. Each round recomputed provider candidates for every retained
boundary by scanning every checked-body machine in the program (core and std
included), and each named-provider test walked every conformance map. The
boundary roster only grows across rounds, and a boundary's candidates depend
only on the checked program, its plans and that boundary, so rounds 2 and 3
repeated round 1's work.

Payload memberships are a separate cost: 26 queries on a 377-block machine
replay 13,038 worklist steps at about 130 us each, 40% of that in `State::meet`.
Frontier reuse already works (2 fresh solves of 26) and backward reachability
is 9 ms. Remaining per-step cost is proposition comparison inside
soundness-critical joins; not changed here.

## Change

`providers::checked_unit_provider_candidates` keeps a per-discovery memo of
each boundary's candidate rows; discovery passes one memo across its rounds.
Rows, their order before the global sort, and every refusal are unchanged.

## A/B

Interleaved, separate warm caches per binary, each binary on its own accepted
copy of the Dungeon (identical sources).

| Run | Baseline wall | Candidate wall |
| --- | ---: | ---: |
| warm, 6 pairs | 11.90-12.86 s, median 12.07 s | 9.49-10.00 s, median 9.58 s |
| warm settle | median 3.06 s | median 1.79 s |
| warm product lowering | median 6.30 s | median 5.06 s |
| cold, 3 pairs | 35.3, 30.5, 30.1 s | 31.7, 31.5, 30.4 s |

Every warm pair favors the candidate by 2.2-2.9 s. Cold is inconclusive at
three pairs: the expected 2.5 s saving is inside the observed cold spread.
Host load differed from the scaling record, so absolute times are not
comparable with it.

Quality: the published `omega-program.exe` is byte-identical between baseline
and candidate (SHA-256 `abf4b8e71320dda8…`) on warm and cold runs, and the
candidate's executable prints the authored opening lines.

Checks, candidate versus unmodified base on the same worktree:
`mbx nextest run -p checked-trees-to-lowered-psi` is 9 of 12 on both (the
same three `source_guards` tests fail on base, naming files this change does
not touch). `corpus_gate.py --filter providers/,boundary/` (338 fixtures)
prints identical per-fixture rows on both: 41 moved, 20 regressed, 24
rejected without their expected fragment, 3 fail fixtures not rejecting,
against the stored golden. Only perf-move counts differ (91 versus 86).

## Next

Round 1 of each discovery still scans all machines per boundary (about 0.6 s
in each of the two large lowerings). Indexing candidate machines by
requirement once per checked program would remove most of it. Payload
membership joins on the two large product machines (4.4 s profiled) are the
largest remaining lowering cost.

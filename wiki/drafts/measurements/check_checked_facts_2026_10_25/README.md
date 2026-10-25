# Checked-fact checks, 2026-10-25

`check_checked_facts` costs about 217 ms per compile — roughly 109 ms per
check — and it is wall time on the calling thread, not summed thread time
like the [std/core machine-checking row](../std_core_machine_check_2026_10_24/README.md).
Three subscopes carry nearly all of it: flow-call borrows (~67 ms), flow-call
contracts (~49 ms), and indexed accesses (~48 ms), each summed over the two
checks in a compile.

Source base `4b80a922ef2fc282a2ed27266b162726703ef1d2`, unmodified. Dev
profile, effective opt-level 1. Binary SHA-256:
`b083b5ad87a58aa1b9d8aadd6bba9e2267a1008a325f56f131a5058d690d7350`. Host:
Windows 11, AMD Ryzen 9 7950X, 32 logical CPUs. Workload
`samples/cli/facts/dyn_dispatch_selector_payload_bound/main.omg`, compiled in
place with `OMEGA_PHASE_PROFILE=1 omega --timings`. Three runs, no warmup;
every run published and exited 0.

## Whole-compile context

| Run | Stage 05 | `check_checked_facts` | Compile |
| --- | ---: | ---: | ---: |
| 1 | 1,695 ms | 217 ms | 2,390 ms |
| 2 | 1,550 ms | 254 ms | 2,694 ms |
| 3 | 1,366 ms | 159 ms | 2,046 ms |

Medians: Stage 05 ≈ 1,550 ms, `check_checked_facts` ≈ 217 ms — about 14% of
Stage 05 and ~9% of the compile. The earlier
[whole-compile profile](../resolution_boundary_2026_10_24/README.md) reported
144 ms for this scope at `9e49ea3065f`; the pass has grown since.

## Where the 217 ms goes

All rows are entered on the calling thread, so each is wall time. Values are
the medians of the three runs, both checks combined (the `2 x` counts mean
each row runs once per check). Rows nest; do not add them.

| Scope | Median | Entries |
| --- | ---: | --- |
| `05.d.7.2.check_flow_call_borrows` | 67 ms | 2 x |
| `05.d.7.3b.check_flow_call_contracts` | 49 ms | 2 x |
| `05.d.7.9.check_indexed_accesses` | 48 ms | 2 x |
| `05.d.7.5_8.crashes_content_carry_operators` | 28 ms | 2 x |
| `05.d.7.4.check_linear_obligations` | 15 ms | 2 x |
| `05.d.7.0.setup_guards_and_entailment` | 6 ms | 2 x |
| `05.d.7.3a.content_composition` | 3 ms | 2 x |
| unscoped tail (stored-integer calls, indexed writes, termination, zero-value qualifications) | ~2 ms | — |

Inside `check_flow_call_contracts` (~49 ms): per-call requirement work over
450 call sites (~21 ms), of which `cap.2d.transitions` over 86 transition
obligations is ~26 ms; exit checking over 297 exits (~18 ms), of which
`exits.a.activation_return` is ~14 ms; domain writes over 386 writes (~6 ms).

Inside `check_indexed_accesses` (~48 ms): `state_argument_facts` is the hot
row — 9 entries ≈ 16–36 ms, ~2–4 ms each — followed by `check_statements`
(51 entries ≈ 9–21 ms). `seed_state` (51 entries ≈ 1.5–2.8 ms) and the
range-call/loop-invariant rows are small.

## Reading

The scope is sequential and already subscoped; nothing here is a parallelism
artifact. If Stage 05 wall time matters, the leverage points in order are
`check_flow_call_borrows` (~31% of the scope), the contract `transitions`
row (about half of `calls` at ~26 ms over 86 obligations), and
`state_argument_facts` (~29 ms over only 9 entries — the most expensive
per-invocation row in the pass, worth a look first since its work is
concentrated in so few calls).

Unattributed remainder is ~2 ms, so the subscope coverage is essentially
complete; no hidden cost lurks inside `check_checked_facts` itself.

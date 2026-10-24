# Flow-fact repetition, 2026-10-24

Flow-fact construction does not repeat a check that already ran. Each check
builds flow facts once. The product check is the cost; the earlier Build check
is a different, smaller dataflow.

This follows the [whole-compile profile](../resolution_boundary_2026_10_24/README.md),
where Stage 05 flow-fact construction was the largest named scope. One compile
of each workload, same host and dev-profile compiler shape as that profile.
`OMEGA_PHASE_PROFILE` was on. A temporary per-check delta, since removed,
printed the change in those rows at the end of each `CheckRequest::check`.
Native publication succeeded for all three. These are single runs, not medians.

The first check is `prepare_checked_build` during Build evaluation. The second
is `check_product_source`.

| Workload | Check | Machines | Check time | Flow facts | Outer iterations | Inner passes | States visited |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| length | Build | 205 | 67 ms | 16 ms | 1 | 33 | 89 |
| length | Product | 103 | 750 ms | 240 ms | 1 | 53 | 339 |
| dynamic dispatch | Build | 205 | 66 ms | 15 ms | 1 | 33 | 89 |
| dynamic dispatch | Product | 103 | 754 ms | 234 ms | 1 | 54 | 339 |
| nested record | Build | 206 | 64 ms | 15 ms | 1 | 33 | 89 |
| nested record | Product | 104 | 712 ms | 232 ms | 1 | 53 | 340 |

The outer fixed-point loop in `build_check_facts` runs one iteration per check.
A second permission, loan, or entry-requirement pass is not scheduled. The
product check visits about 340 states across about 54 inner passes, roughly
six states per pass, which matches grouped sweeps plus a confirming pass rather
than a full-program rebuild on every pass. The Build check reaches 89 states
in 15 ms. Replaying it would not remove the product check's 230 ms.

## Answered

The state visit and the per-group copy are separated in
[the saturation record](saturated.md). The product-check wall is the slowest
group's sweep. That group's baseline copy is about 1 ms.

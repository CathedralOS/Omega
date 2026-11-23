# Readiness comparison

Blocking cases: 3 → 2.

| Blocking cases | Earlier | Later |
| --- | ---: | ---: |
| Blocked in both, same or unrecorded programs | 1 | 1 |
| Measured on a different program | 1 | 1 |
| No longer blocking while still measured | 1 | — |
| Blocked before and no longer measured | 0 | — |
| Regressions of previously passing cases | — | 0 |
| Newly measured | — | 0 |
| Total | 3 | 2 |

Blocked in both: 0 with a changed first stop, 0 with additional refusal fixtures, 1 with a changed emitting test, 0 with changed text only, 0 unchanged. No longer blocking: 0 with every earlier program still recorded, 1 where a report does not record the program. Regressions: 0 with every earlier program still recorded, 0 where a report does not record the program.

| Report | Revision | Fingerprint | Layers | Cases | Blocking | Incomplete measurements |
| --- | --- | --- | --- | ---: | ---: | ---: |
| 20261121T111131Z-pxf_dgd1 | e1c9fb0255e4 | 7e3cf3eb2109 | through L5 | 3 | 3 | 0 |
| 20261121T114344Z-wxz4b6lj | e1c9fb0255e4 | d6b2b187f128 | through L5 | 3 | 2 | 0 |

## Same compiler stop, different emitting test (1)

- handoff/arithmetic on float reads
  - test: tests/recasts.rs::float_views_do_not_admit_mutable_homes_owned_bit_casts_or_float_arithmetic → tests/recasts.rs::float_views_do_not_admit_owned_bit_casts_or_float_arithmetic

## Measured on different programs under the same case key. Neither report shows how the other program behaves, so these are not same-input regressions, stop changes or repairs (1)

- handoff/carry policy on a transparent data alias · was L5 · unresolved refusal · transparent alias carry policy requires a checked relation to its expansion · now L5 · unresolved refusal · transparent alias carry policy requires a checked relation to its expansion
  - blocks: blocks → blocks
  - inputs: prepared alias.omg sha256:ba29e38372749c25 → prepared alias.omg sha256:cca52b69c1b1b9d9
  - locations: app/alias.omg:2:12 bytes 21..92 "[copy,carry(suspension:forbidden,cpu:same,thread:any,address:movable,)]" → app/alias.omg:2:12 bytes 21..89 "[copy,carry(suspension:allowed,cpu:any,thread:any,address:movable,)]"
  - earlier inputs in the later report: prepared alias.omg sha256:ba29e38372749c25 → sha256:ba29e38372749c25 no fixture records this source; successful fixtures record none

## Blocked before, measured now, and no longer blocking (1)

- handoff/cleanup in a ranked component · was L5 · checker input blocked · ranked recursion needs a terminal call or a non-suspending direct scalar call · now L5 · checker input compared; judgement pending
  - inputs: prepared ranking.omg sha256:ea28aa43237a89e3 → not recorded
  - at app/ranking.omg:2:46 bytes 85..114 "{ let lease=Lease {mark:0}; }"

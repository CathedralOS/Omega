# Sample gate, macOS arm64

`python3 tools/sample_gate.py` runs `omega --check` over every `samples/**`
project carrying a `build.omg`, except `apps/squalr`. This file records what
it said and what MOVED, so the next reader does not re-derive the delta from
a stale board.

## 2026-09-27 at `f7cbf4e93375`, plus the toolchain-library recovery

**105/148 check, 43 refuse.** Against 89/148 at `9246f32b305` earlier the same
day: **+16, zero regressions.**

| cluster | was (89) | now (105) |
| --- | --- | --- |
| implicit store conversion | 17 | **0** |
| unit-plan omission | 22 | 18 |
| provider absent on this host | 11 | 11 |
| nested value call | 5 | 5 |
| boundary provider route unimplemented | 3 | 3 |
| default-domain field requirement | 1 | 2 |
| other | 0 | 4 |

**IMPLICIT-STORE-CONVERSION's sample cluster is empty.** It was the second
largest and is now fully cleared, by the sample-side conversion spellings and
the compiler work that landed beside them. `unit-plan omission` fell 22 -> 18,
concentrated in the guard-expression phase (6 -> 3) that the mixed integer
comparison repair reached.

The 4 in `other` are new only as a label: one is `dungeon_crawler_cli`, which
advanced out of implicit-store into an honest `exact arithmetic ... may
overflow u64` proof obligation, and three are the GUI samples reporting
`validated named operator application has no checked selected use`.

### Read this before trusting any earlier number

This run was taken **after** repairing a stale baked library path. The first
attempt the same day reported **55/148**, which reads as a 34-sample
regression and was not one: 77 samples failed with `missing Omega toolchain
library at <repo>/.codex/worktrees/bootstrap-test-cleanup/source/library`, a
worktree that no longer exists. `build-evaluation/build.rs` bakes that absolute
path at compile time and asks Cargo to rerun only when the release digest
changes, so it survives the checkout being moved or deleted, and a rebuild
immediately beforehand does not clear it.

The `9246f32b305` reading above is **not** affected — its `other` cluster was
empty, which is the signature of this bug — so the 89 -> 105 gain is real.
When a reading drops by tens of cases, cluster the diagnostics before
reporting a regression.

## 2026-09-27 at `9246f32b305`

**89/148 check, 59 refuse.**

Against the previous reading at `de525cf687d`, 117 commits earlier:
**85 -> 89, net +4, and ZERO regressions.**

| sample | why it moved |
| --- | --- |
| `cli/basics/brightness_control` | the field-subject dispatch chain was repaired; its board item is retired |
| `cli/simulation/grid_walk` | the member-policy RECORD half was repaired by `ad4873116ae`, which also removed its pin |
| `cli/systems/file_permissions` | sample edit: typed the anonymous bitwise operand its diagnostic names |
| `cli/text/text_padding` | sample edit: spelled the carrier-length conversion |

Two compiler repairs and two sample edits, so the tree is closing from both
ends. The member-policy ARRAY half is NOT repaired -- `collections/matrix_multiply`
and `rendering/dungeon_render` both still omit at `structural field store:
scalar field type`.

## Clusters

| cluster | count |
| --- | --- |
| unit-plan omission | 22 |
| implicit store conversion | 17 |
| provider absent on this host | 11 |
| nested value call | 5 |
| boundary provider route unimplemented | 3 |
| default-domain field requirement | 1 |

`other` is now EMPTY -- it held the two anonymous-bitwise samples and both
are repaired -- and so is `non-tail self recursion`. `provider absent on this
host` rose 10 -> 11 because `cellular_automaton` advanced INTO it once its
bitwise operand was typed: an honest advance into a host-gated cluster, not a
regression.

## unit-plan omission, by phase

| count | phase |
| --- | --- |
| 6 | `state graph: terminator: conditional successors: guard expression` |
| 3 | `statement sequence: local data: structural call binding` |
| 3 | `structural field store: byte sequence carrier` |
| 3 | `call operation: boundary arguments` |
| 2 | `structural field store: pure source: scalar expression row` |
| 2 | `structural field store: scalar field type` |
| 1 | `state graph: prefix initializers: bound expression` |
| 1 | `scalar field store sequence: write frame agreement` |
| 1 | `state graph: terminator: conditional successors: parameter transfer` |

## Reading this later

Rebuild the CLI at the commit you measure. A reading taken with a stale
binary is not a reading of that commit: measured the same day, a nine-hour-old
binary reported a gap a freshly built one showed repaired. With the CLI
already built the run costs about fifteen minutes.

Reclaim disk BEFORE starting it. The run compiles 148 projects in parallel;
begun at 16 GiB free it exhausted the volume and stopped the shell. See
`tools/testing.md` and prune `target/debug/incremental` while the tree is idle.

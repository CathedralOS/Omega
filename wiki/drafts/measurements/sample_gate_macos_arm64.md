# Sample gate, macOS arm64

`python3 tools/sample_gate.py` runs `omega --check` over every `samples/**`
project carrying a `build.omg`, except `apps/squalr`. This file records what
it said and what MOVED, so the next reader does not re-derive the delta from
a stale board.

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

# Sample gate, macOS arm64

`python3 tools/sample_gate.py` runs `omega --check` over every `samples/**`
project carrying a `build.omg`, except `apps/squalr`. This file records what
it said and what MOVED, so the next reader does not re-derive the delta from
a stale board.

## 2026-09-27 at `695b5e05d07`, after the Clock provider wiring

**119/148 check, 29 refuse. +14 against 105 earlier the same day, zero regressions.**

| cluster | at 105 | now |
| --- | --- | --- |
| unit-plan omission | 18 | 15 |
| nested value call | 5 | 5 |
| other | 4 | 4 |
| boundary provider route unimplemented | 3 | 3 |
| provider absent on this host | 11 | **2** |
| default-domain field requirement | 2 | **0** |

Two clusters closed. **`default-domain field requirement` is empty**:
`print_number` and `maze_flood` both stored a computed `u8` into a carrier
whose domain requires `valid_utf8(self)`, and each computed write is now
dominated by a guard proving the byte is ASCII. **`provider absent on this
host` fell 11 -> 2**: all nine samples in SAMPLE-CORPUS's "Clock-bearing
samples" row are wired to the declared-host darwin provider.

The 14 gained are those 11 plus `text_greeting`, `status_report` and
`string_catalog`, which moved under another lane's byte-carrier work.

`elapsed_timer` and `file_journal` are what remain in `provider absent`: they
want `TimeHost` and `Filesystem` bindings, not `Clock`.

### The Clock row was NOT host-gated, and reading the board is what settled it

Measured first, wrongly, as "correctly host-gated, implementation-blocked on a
backend `sleep_poll` leg". `source/library/std/clock.omg` does decline the
`darwin_poll` adapter, because `invokes DarwinClock` would leak into every
consumer's `reaches` — but that is a decision about the PORTABLE `std` Clock.
These nine declare package-local traits, where the board explicitly directs
the `samples/gui/window_app` pattern and notes that "missing host selection is
distinct from a lowering failure". The leak is real and is accepted per
sample; `Main::main` now publishes `DarwinClock`.

Their linux and windows legs remain unwired, exactly as before the change.
Completing those wants `std::clock`'s `nanosleep`/`Sleep` closures rather than
nine per-sample copies.

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

### `dungeon_crawler_cli` is at least two blockers deep

`carve_room` does `level.room_count = max(level.room_count, index + 1)` where
`index` binds `self.cell_index(cell)`, and `cell_index` carries
`ensures result <= 15`. The bound is real, so the addition cannot overflow a
`u64`, yet the assignment is refused as not provably in range.

Constraining the local (`let index: u64 [0..=15] = ...`) does clear that
refusal, and the sample then advances to a SECOND blocker: the unit plan is
omitted at an unavailable callee `Game::run_game_loop`, itself omitted at
`call operation: structural arguments: parameter path`. So clearing the
arithmetic does not make this sample check, and that edit was reverted rather
than landed: **REMOVE-BRACKETED-RANGE-ANNOTATIONS is migrating ~1,300 sites
off `T [lo..=hi]`**, and a load-bearing local bound is exactly the row that
memo records as NOT migrating to `in D`, so adding one here buys an
intermediate blocker at the cost of new migration debt.

The open question is why the `ensures` does not reach the caller's
arithmetic. Four reductions all CHECK — a call result plus one, a literal
control, the arithmetic in a call-argument position, and a store through a
nested field — so the callee-bound-to-caller-arithmetic route works
generally, and something specific to this sample defeats it. Worth noting
before re-deriving: in those reductions the callee's arms are constants, so
the bound may come from the BODY rather than the `ensures`; a reduction whose
callee is bounded ONLY by its `ensures` is the next control.

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

## The long checks have moved, 2026-09-28 at `1bfa4c6351c`

SAMPLE-CORPUS asks to "reproduce the reported long checks for
`gui/windowed_calculator` and `cli/simulation/game_of_life_glider` on a
current binary and locate the cost". Reproduced: **they are no longer the long
checks.** One `omega --check` each, sequential, nothing else running:

| sample | `--check` | against baseline |
| --- | --- | --- |
| `collections/matrix_multiply` | **2232.3s** (37 min) | 31x |
| `systems/descriptor_walk` | **805.1s** (13.4 min) | 11x |
| `simulation/game_of_life_glider` | 91.7s | 1.3x |
| `gui/windowed_calculator` | 75.6s | 1.1x |
| `basics/cli_mvp` (trivial control) | 71.0s | baseline |

The two the board names sit within 30% of a sample that sums a few integers
and exits, so on a current binary they carry no measurable extra cost. The two
that do are `matrix_multiply` and `descriptor_walk`, and neither appears in
that line.

Both slow samples were discovered as **timeouts**, not as slow runs:
`tools/sample_native_gate.py` at a 300s budget reported them as
`review-timeout`, which is how a 37-minute check presents itself to any gate
with a per-call budget. A gate that reports a timeout is not reporting a
compiler refusal, and the two must not be pooled.

**The 71s baseline is the other half of the story.** A trivial sample costs
over a minute, so most of every sample check here is fixed cost, not program
cost, and the corpus-wide gate is dominated by it. Locate the cost in
`matrix_multiply` against that floor: the interesting quantity is the 2161s
above baseline, not the 2232s total.

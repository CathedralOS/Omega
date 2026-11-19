# Sample status, 2026-11-16

On current `main`, 134 of the 374 sample and fixture programs that should run
do run, with the native program and the Psi interpreter agreeing on the
expected exit. At the start of this wave (2026-11-15) 26 did. The other 240
are still refused. One intended-invalid program is now wrongly accepted and
needs fixing first (see [Wrong accepts](#wrong-accepts)).

This is a one-time measurement, not a gate. It records where the samples
stand so the next repairs can start with the most common refusals. Each
program was run with `omega run --both <main.omg>`, which compiles the program,
runs it natively and in the Psi interpreter, and compares the two exits.

- **Before:** compiler built at `372860cf534b` (the start of this wave). Each
  program was run from that commit's own checkout, so a program that was
  migrated or added later is either refused in its old form or counted as
  absent.
- **After:** compiler built at `b3f4750fffc6` (`origin/main` on 2026-11-16),
  run from that checkout.

## Counts per directory

"Runs" means both engines exit with the exit the program states
(`// Expected exit: N`), or agree on an exit when the program states none.
Every agreeing run matched its stated exit, and native/Psi never disagreed.
"Controls refused" counts intended-invalid programs that the compiler
correctly refuses. "Absent before" counts programs that did not exist at
`372860cf534b`. No run reached the 900 s timeout.

| Directory | Entries | Runs (before → after) | Controls refused | Refused | Wrong accepts | Absent before |
| --- | --- | --- | --- | --- | --- | --- |
| `samples/cli/algorithms` | 13 | 0 → 1 | 2 → 2 | 11 → 10 | 0 → 0 | 0 |
| `samples/cli/arithmetic` | 24 | 0 → 10 | 1 → 1 | 23 → 13 | 0 → 0 | 0 |
| `samples/cli/audio` | 5 | 0 → 0 | 0 → 0 | 5 → 5 | 0 → 0 | 0 |
| `samples/cli/basics` | 11 | 1 → 4 | 0 → 0 | 10 → 7 | 0 → 0 | 0 |
| `samples/cli/collections` | 19 | 0 → 5 | 2 → 2 | 17 → 12 | 0 → 0 | 0 |
| `samples/cli/games` | 8 | 0 → 4 | 0 → 0 | 8 → 4 | 0 → 0 | 0 |
| `samples/cli/interpreters` | 8 | 0 → 3 | 0 → 0 | 8 → 5 | 0 → 0 | 0 |
| `samples/cli/proofs` | 9 | 0 → 2 | 1 → 1 | 8 → 6 | 0 → 0 | 0 |
| `samples/cli/rendering` | 11 | 0 → 2 | 0 → 0 | 11 → 9 | 0 → 0 | 0 |
| `samples/cli/scientific` | 210 | 4 → 10 | 71 → 71 | 135 → 129 | 0 → 0 | 0 |
| `samples/cli/simulation` | 15 | 0 → 6 | 0 → 0 | 15 → 9 | 0 → 0 | 0 |
| `samples/cli/support` | 4 | 0 → 0 | 0 → 0 | 4 → 4 | 0 → 0 | 0 |
| `samples/cli/systems` | 20 | 0 → 8 | 3 → 2 | 17 → 9 | 0 → 1 | 0 |
| `samples/cli/text` | 20 | 0 → 4 | 1 → 1 | 19 → 15 | 0 → 0 | 0 |
| `tests/fixtures/run/arithmetic` | 1 | 0 → 1 | 0 → 0 | 0 → 0 | 0 → 0 | 1 |
| `tests/fixtures/run/calls` | 13 | 7 → 12 | 0 → 0 | 1 → 1 | 0 → 0 | 5 |
| `tests/fixtures/run/contracts` | 1 | 0 → 1 | 0 → 0 | 0 → 0 | 0 → 0 | 1 |
| `tests/fixtures/run/domains` | 3 | 0 → 3 | 0 → 0 | 0 → 0 | 0 → 0 | 3 |
| `tests/fixtures/run/effects` | 1 | 0 → 1 | 0 → 0 | 0 → 0 | 0 → 0 | 1 |
| `tests/fixtures/run/equality` | 2 | 0 → 2 | 0 → 0 | 0 → 0 | 0 → 0 | 2 |
| `tests/fixtures/run/float` | 10 | 0 → 10 | 0 → 0 | 0 → 0 | 0 → 0 | 10 |
| `tests/fixtures/run/generics` | 9 | 6 → 9 | 0 → 0 | 0 → 0 | 0 → 0 | 3 |
| `tests/fixtures/run/layouts` | 1 | 0 → 1 | 0 → 0 | 0 → 0 | 0 → 0 | 1 |
| `tests/fixtures/run/operators` | 3 | 1 → 2 | 0 → 0 | 1 → 1 | 0 → 0 | 1 |
| `tests/fixtures/run/ownership` | 19 | 4 → 19 | 0 → 0 | 0 → 0 | 0 → 0 | 15 |
| `tests/fixtures/run/proofs` | 1 | 0 → 1 | 0 → 0 | 0 → 0 | 0 → 0 | 1 |
| `tests/fixtures/run/providers` | 3 | 0 → 2 | 0 → 0 | 1 → 1 | 0 → 0 | 2 |
| `tests/fixtures/run/reflection` | 1 | 0 → 1 | 0 → 0 | 0 → 0 | 0 → 0 | 1 |
| `tests/fixtures/run/views` | 10 | 3 → 10 | 0 → 0 | 0 → 0 | 0 → 0 | 7 |
| **Total** | 455 | 26 → 134 | 81 → 80 | 294 → 240 | 0 → 1 | 54 |

No program that ran at `372860cf534b` stops running on `main`. The large
before-refusal counts mostly come from two errors that later PRs removed from
the samples: "a data declaration cannot own a machine" (149 programs, which
the static-machine migration rewrote) and "import does not select one
declaration" (82 programs).

**Checker-only fixtures** (not counted above). Four fixtures in
`tests/fixtures/run/calls/` have no `build.omg`; their test harness supplies
the entry itself. Run on their own, they are refused on both commits:
`never_returning_call_in_expressions`, `non_returning_call_lends_storage` and
`statement_after_process_exit` with "executable product has no selected
program entry". On `main`, `process_exit_conditional_helper` is refused
earlier, with "not supported yet: typing arm value type".

## Top refusals on main

Each message below has its names and numbers replaced with `_` and `N`. The
240 refused programs produce 58 distinct first errors. These are the 15 most
common.

| Programs | First error | Examples |
| --- | --- | --- |
| 34 | root binding `_` is ambiguous within its package | `collections/disjoint_union_find`, `scientific/antenna_array_pattern` |
| 33 | this value's type carries no domain and the parameter it is passed for declares `_` (a domain is gained only through `as +D` / `as -D`) | `algorithms/longest_run`, `algorithms/maze_flood` |
| 18 | this value's type carries no domain and the place it is assigned to declares `_` | `algorithms/binary_search_viz`, `audio/scale_melody` |
| 16 | not supported yet: a call of a machine whose signature is refused above | `rendering/bouncing_ball_2d`, `scientific/bounded_state_estimation` |
| 13 | not supported yet: call argument that names storage another argument lends | `algorithms/horner_eval`, `algorithms/topological_order` |
| 10 | this expression has no visible value declaration in its source scope | `arithmetic/fibonacci_golden`, `rendering/mandelbrot` |
| 9 | not supported yet: type arithmetic policy on a type that is not an integer | `arithmetic/fixed_limb_bignum`, `arithmetic/prime_sieve` |
| 7 | this write opens the invariant window: it is not shown to keep requires clause N of a data type | `algorithms/bubble_sort`, `algorithms/dutch_flag` |
| 7 | reviewed native production requires application root `_`; found Package | `audio/audio_playback`, `audio/wav_output` |
| 6 | not supported yet: declaration holding a call whose name resolves to no declaration | `arithmetic/sensor_min_max`, `arithmetic/vector_distance` |
| 5 | not supported yet: typing argument types | `algorithms/record_range_search`, `algorithms/sort_visualizer` |
| 5 | not supported yet: expression range | `arithmetic/recursive_sum`, `collections/slice_maximum` |
| 5 | exact arithmetic may overflow `_`: the operands are not provably in range | `arithmetic/smallest_prime_factor`, `collections/heat_grid` |
| 5 | cannot prove index is within length N | `collections/array_sum`, `collections/entity_list` |
| 5 | cannot prove requires contract for call | `collections/inventory_system`, `games/tic_tac_toe` |

Paths are relative to `samples/cli/`. The first error comes after the generic
"cannot compile admitted project" wrapper line.

## Wrong accepts

- `samples/cli/systems/file_journal/invalid_inference_cycle` runs and exits 0
  on both engines. Its `sample.json` marks it intended-invalid, with the
  expected refusal "omits `invokes Third;`". The machine `a` declares
  `invokes First` and `invokes Second`, but it reaches `Third::value()`
  through the cycle `a → b → c → third()`. The checker does not carry the
  boundary requirement of a recursive helper back around the cycle to `a`.
  The program's own header also says `// Expected exit: 0`, which contradicts
  `sample.json`. At `372860cf534b` the compiler refused the program for an
  unrelated reason: "call boundary requirement must declare its crash
  ceiling".

## Method

Program roots are every `main.omg` under `samples/cli/*/`, outside `facts/`
and `probes/`, and every `tests/fixtures/run/**/main.omg`. Under
`samples/cli/`, standalone `controls/` programs without a `build.omg` are
counted too. A program is intended-invalid by the rule
`tools/xtask/src/samples.rs` (`project_at`) uses: a directory named `invalid`,
`intended_invalid` or `out_of_bounds`, or one starting with `invalid_`, a path
through `fail/`, or a `controls/` directory that holds an expected-diagnostic
file. Each run was `timeout 900 omega run --both <main.omg> < /dev/null` from
the checkout root, with at most 3 runs in parallel on a 4-core cloud
container. Both compilers were built with
`CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p omega`.

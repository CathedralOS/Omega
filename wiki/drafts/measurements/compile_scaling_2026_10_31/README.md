# Compile scaling: fixed library cost versus program cost

2026-10-31, Windows x86-64, Ryzen 9 7950X (16 cores), shared host at about 50%
CPU load from other sessions. Release compiler (`opt_level` 3) built from main
`62ce026b593`, SHA-256 `60247a071d05f9ed7ea499a2a02cf769cb8ce8253cf29c40189fd83202d1724e`.
**Decision: retain the measurement; no compiler change.**

The question: how much of a Dungeon compile is cost every program pays, and how
much grows with the program? Three programs of increasing size, compiled with
`omega --timings` (compile only, `windows_x86_64`):

| Program | Source beyond libraries |
| --- | --- |
| `empty` | 3 lines, no dependency (implicit `core` only) |
| `hello` | 6 lines, depends on std, calls `Console::write_line` once |
| `dungeon` | `samples/cli/games/dungeon_crawler_cli`, 3,152 lines, depends on std |

`core` is 11,458 lines and std is 19,069 lines.

## Results

Three interleaved rounds. Each round compiles every program once with a fresh
private `LOCALAPPDATA` cache (cold), then twice more on that cache (warm). Wall
time is external; the compiler's own `total elapsed` agrees within 0.2 s on every
run. Raw rows are in [runs.txt](runs.txt).

| Program | Cold wall, median of 3 | Warm wall, median of 6 | Cold Stage 05 (checking), median |
| --- | ---: | ---: | ---: |
| empty | 2.37 s | 0.69 s | 1.54 s |
| hello | 14.30 s | 3.53 s | 6.99 s |
| dungeon | 26.52 s | 9.34 s | 13.10 s |

Round 1 is slower for every program (empty cold 3.90 s versus 2.31/2.37 s),
consistent with first-touch disk and page cache effects. Host load varied; these
are not a controlled A/B and must not be compared with earlier records' times.

Representative stage breakdown (round 2). Rows are the CLI's own scopes, not a
partition; only those above 50 ms (warm) or 200 ms (cold) are shown.

| Stage | empty warm | hello warm | dungeon warm | hello cold | dungeon cold |
| --- | ---: | ---: | ---: | ---: | ---: |
| Stage 05: TypedTrees -> CheckedTrees | 0.003 | 0.016 | 0.030 | 6.99 | 13.99 |
| settle: CheckedTrees -> SelectedExecution | 0.31 | 1.49 | 2.04 | 1.49 | 2.07 |
| terminal-production (lowering) | <0.05 | 0.98 | 4.44 | 1.04 | 4.80 |
| total elapsed | 0.54 | 3.31 | 8.58 | 14.10 | 26.78 |

## What this shows

- **std is half of a cold Dungeon compile.** A one-call program that imports std
  takes 14.3 s cold; the Dungeon's 3,152 lines add 12.2 s on top. Most of std's
  share is checking: Stage 05 goes from 1.5 s (core alone) to 7.0 s (with std).
  Psi checks every supplied implementation by design, and that work is identical
  for every program on one compiler and std revision.
- **Warm compiles redo lowering and settlement.** Checking is reused (Stage 05
  falls to 16-30 ms), but with no source change the Dungeon still spends 4.4 s
  lowering and 2.0 s settling: 75% of its 8.6 s warm compile. Even `hello` spends
  1.0 s lowering and 1.5 s settling warm.
- **The empty program is already sub-second warm** (0.54-0.69 s). The fixed
  pipeline floor is not the obstacle to a sub-second edit loop. The obstacle is
  work proportional to reachable program and library bodies that is recomputed
  even when its inputs did not change.

For a sub-second Dungeon compile, the two levers are therefore different by
scenario:

- Edit-recompile loop: reuse lowered and settled results for bodies whose
  checked inputs are unchanged, as checking already is. That targets about
  6.5 of 8.6 warm seconds.
- Cold compile: reuse checked std and core across projects (the input is the same
  library revision and compiler), or shrink std checking. That targets about
  5.5 s of the 13-14 s of cold checking before touching the program's own cost.

Neither has been implemented or measured; these are the next hypotheses, not
predicted savings.

## Measurement notes

- The step labelled `build: AssembledSyntax -> CheckedTrees (contains Stage 01-05)`
  reports 1.0 s on a cold Dungeon run while Stage 05 alone reports 14.0 s. The
  label's containment claim does not hold for this route; do not use it to
  subtract or partition checking time.
- On the same afternoon, before load fell, launching any process took 2-10 s
  (`cmd /c echo` 2.7-10.3 s, `omega --version` 2.0-9.1 s). Every external wall
  time from that period was discarded. The runs above started after
  `omega --version` fell to 0.1-0.5 s.
- A dev-profile producer (`opt_level` 1) measured earlier the same day gave
  empty 4.17 s cold and 0.56 s warm (self-reported) under heavier load. No
  matched dev-versus-release comparison was made.

## Setup and controls

Both std-using probes are copies under the worktree's ignored `build/pscale/`.
Their package reviews (358 decisions each: 172 external supplies, 172 callables,
10 dangerous capabilities, 3 terminal permissions, 1 native binding, all for
bundled std on `windows_x86_64`) were accepted with owner approval through the
normal `omega update` / `--resume` route. Accepted lock SHA-256: hello
`b8cdcb694f80519e75b8ea7fcc3f35adb08bce49146a6714695dbd3250b2cffb`, dungeon
`5136c3d19ecc69c55b3c5858fde83b5597ae32c7d19539f9ab83acbc2da98fdc`. The tracked
Dungeon `omega.lock` was not changed. The Dungeon copy differs from the sample
only in its std path depth.

Before timing, `omega run` on the Dungeon copy exited 0 with the authored
`A bottomless dark room near the dungeon heart.` line, and `hello` printed
`hello` with exit 0. All 27 timed compiles exited 0.

The probes are in [probes/](probes/); place them at `build/pscale/<name>/` to
reuse their std path, as [run.sh](run.sh) expects. On main `1aa92f8aae6`, std
did not check for Windows (`Clock` had four covering provider plans); #1065
resolved that before this measurement.

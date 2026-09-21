# NEW-NFF-STD-CHECK-DURATION-RECORD — `omega --check` over the std package, linux x86-64

Measurement record for the direct `omega --check` probe of the bundled
standard-library package (`source/library/std`, package
`omega-language-std`) on this session's linux x86-64 host (8 CPUs,
`uname -m` = x86_64). Recorded 2026-09-21 at worktree revision
`37bcdfa0ccc0e` (origin/main `5bb9a74842dd1` plus one board-stamp commit);
the `omega` binary was rebuilt immediately before measurement
(`cargo build -p omega`, dev profile, 59.1s).

| Command | Wall clock | User time | Exit | Outcome |
| --- | --- | --- | --- | --- |
| `omega --check --timings --offline source/library/std/main.omg` | 15:44.98 (944,969.8 ms) | 943.56s (99% CPU) | 1 | fails inside `omega-language-std` checking |
| `omega --check --timings --offline --target linux_x86_64 source/library/std/main.omg` | 15:27.47 (927,460.2 ms) | 927.29s (99% CPU) | 1 | same diagnostic |

## Diagnostic (both runs' expected terminal state)

```text
cannot check prepared project: checked compilation failed for package `omega-language-std` with 1 diagnostic(s)
  error: routed service field `Filesystem::host` has no exact Fused selected-provider-plan join
```

This is the known `Filesystem::host` join regression already on the board
(`NEW-NFF-FILESYSTEM-HOST-PLAN-JOIN-REGRESSION`, minted by `2704dd0edbf2`,
`selected-dispatch/src/boundary_dispatch.rs:229`). The sample-level probes
recorded there (`multiplication_table` ~25m, `dungeon_crawler_cli` 27m29s,
both `--target linux_x86_64`) spend that time inside the same
`omega-language-std` package compilation; the direct package probe above
reaches the same diagnostic in ~15m45s, so ~10–12 minutes of each sample
probe is package-dependent entry/binding work, not std checking itself.

## Method notes

- `/usr/bin/time -v` wraps the process; peak RSS 198 MB; zero major page
  faults; the run is CPU-bound single-threaded-adjacent (99% of one core —
  checking is not parallelized across the package closure).
- `--offline` pins package acquisition to local sources; the full
  `use calling; use console; use filesystem; ...` module closure is checked.
- The check is *not* a hang: it terminates with the diagnostic above.
- `--timings` emitted only the total-elapsed line on the failing path
  (944,969.849 ms); per-stage lines are not produced once the package
  compilation aborts.

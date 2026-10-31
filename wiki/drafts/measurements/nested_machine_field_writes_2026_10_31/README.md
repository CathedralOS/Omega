# Nested-machine field-write checking cliff, 2026-10-31

`samples/gui/windowed_calculator` (262 nested machines) cannot complete
`omega --check` in 15 minutes at ~800% CPU / 1.5 GB RSS on a 16-core Linux
host. `samples/gui/window_app` (39 nested machines) checks in 6.7 s with
Stage 05 (`TypedTrees -> CheckedTrees`) carrying 5.2 s. The wall-time driver
is inside `psi_compiler::CheckRequest::check`, not parsing or settlement.

A minimal reproducer shows the blowup is **super-quadratic** and tracks
per-machine field writes into the owning data record, not raw machine count.

## Reproducer

`probes/isolate.py N writes guard out.omg` generates `machine
Main::main(main: &mut Main)` holding N nested machines; each machine does
`writes` field writes (`main.x = main.x + 1`, `main.y = main.y * 2`) and a
guarded transition to the next machine. A copy lives at
`~/Omega-prodreq/probes/` on `omega-linux` (worktree at the #999 merge head).

## Scaling (writes=2, guard `x < 4000 && y > 0`)

| Nested machines | Wall | Marginal over ~1.7 s std baseline |
|---|---|---|
| 32 | 2.9 s | ~1.2 s |
| 64 | 18.8 s | ~17 s |
| 128 | 285 s | ~283 s |
| 256 | not finished | extrapolates to ~75+ min |

~15x marginal cost per doubling — worse than quadratic. Trivial nested
machines (no writes, unconditional transition) stay flat at ~0.3 s marginal
even at N=256, so machine count alone is not the driver.

## Isolation at N=64

| Field writes/machine | Guard | Wall |
|---|---|---|
| 2 | two conjuncts | 18.8 s |
| 2 | one conjunct | 28.7 s |
| 1 | two conjuncts | 10.5 s |
| 0 | two conjuncts | 3.6 s |

Each field write into `main` adds ~8-13 s at N=64; guard width is not the
driver (one conjunct measured *slower* than two). Hypothesis: each write's
established fact set is re-derived or re-proven per successor machine, and
the proof enumerates combinations of accumulated `main` facts.

## Blocks

`samples/gui/windowed_calculator` checking on every host — the GUI
provider-selection acceptance under PROVIDER-SELECTION-AFTER-TERMINAL
cannot reach `run` on any target until checking completes.

## Next step

Instrument or bisect Stage 05 for nested-machine field-write fact
establishment: find where a nested machine's write transports facts into
the owner context and whether each successor re-enumerates the accumulated
owner fact set. Probe sizes N in {32, 64} reproduce the cliff in < 30 s.

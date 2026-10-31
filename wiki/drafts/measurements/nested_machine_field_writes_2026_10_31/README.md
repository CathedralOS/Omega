# Nested-machine transition-path explosion, 2026-10-31

`samples/gui/windowed_calculator` (262 nested machines) cannot complete
`omega --check` in 15 minutes at ~800% CPU / 1.5 GB RSS on a 16-core Linux
host. `samples/gui/window_app` (39 nested machines) checks in 6.7 s with
Stage 05 (`TypedTrees -> CheckedTrees`) carrying 5.2 s.

The blowup is **O(N^3) callee re-traversal against O(N)-growing per-path
judge state** — cubic event count times linear-per-event cost, with a
swap-thrash wall-time knee once RSS crosses the 12 GB host budget
(N=40 probe: 11.5 GB RSS). Localized by `OMEGA_PHASE_PROFILE=1 --timings`
plus a `OMEGA_PATH_COUNT=1` event counter on minimal reproducers.

## Reproducer

`isolate3.py N mode out.omg` (in this folder, also `~/Omega-prodreq/probes/`
on omega-linux) generates `machine Main::main(main: &mut Main)` holding N
nested machines in a call chain; each does two field writes and an
unconditional transition-call to the next machine. `mode` = `const` |
`bounded` (`%256` arithmetic) | raw (`x+1`, `y*2`).

## Scaling (bounded, cold — delete compiled-review-cache between runs)

| N | Wall | path events (OMEGA_PATH_COUNT) |
|---|---|---|
| 24 | 0.14 s | |
| 26 | 9.3 s | 28,264 |
| 28 | 37.3 s | 34,536 |
| 30 | >240 s | |
| 32 | 182 s | |
| 48 | 158 s | |
| 64 | 208 s | |
| 128 | 225 s | |

`const` writes stay flat at N=128 (2.8s) — arithmetic field writes in the
call chain are required, not machine count and not guard width.

**Event counts are cubic, not exponential.** Per-depth counts at N=26 are
exactly `8 * T(N-d+1)` (T = triangular number): depth 26 logs 8 events,
depth 25 logs 24, depth 24 logs 48 — summing to ~`(4/3)N^3`. m28/m26 event
ratio 1.22 matches `(28/26)^3 = 1.25`. Each level re-enters
`recognize_structural_state_leaves` for its callee ~8 times (the probe's
guard fork x two write-value paths), and every re-entry recursively
re-checks the whole machine suffix — no memoization across the sibling
path variants, so depth-d work repeats T(N-d) times.

**Wall time is worse than cubic** because each path event clones and
normalizes a judge carrying every write fact accumulated along the chain
(O(N) state per event, O(N^4) total work, O(N^2+) resident memory). The
apparent "exponential knee" at N ~ 26-30 is the box crossing into swap:
RSS grows quadratically and past ~12 GB the host thrashes, stretching
m28's ~40s of algorithmic work into >240s at m30, then plateauing at
swap-limited ~160-230s.

WARNING: `compiled-review-cache` under `~/.cache/CathedralOS/` caches check
results; repeat runs measure 0.15s cache hits. Clear it or append a unique
source comment per measurement.

## Root cause (phase profile, N=28 cold)

```
2558028.838 ms  17793 x  05.d.7.3b.1.cap.2d.transitions   <- dominant
 101358.602 ms    236 x  05.d.2.a.2b.state_statements
 100296.370 ms    888 x  05.d.2.a.2b.5.validate_statement_node
 100256.665 ms     31 x  05.d.2.a.2.machine.user_or_other   (3.2s/machine)
```

`05.d.7.check_checked_facts` → `structural_case_arms.rs:1689` wraps the
transition-arm expansion:

```rust
let mut remaining_paths = vec![(judge.clone(), case_equations.clone())];
for (statement_index, transition) in transitions {
    for (incoming, remaining_equations) in remaining_paths {
        for mut outcome in incoming.normal_return_paths(...) { ... }
    }
}
```

Per selected path, `normal_operation_entries` /
`normal_argument_entries` produce `target_frames` (measured: 1 frame per
call in this probe), then `recognize_structural_state_leaves` recurses
into the callee (line ~2005) and `leaves.extend(completed)` carries the
callee's whole outcome set upward. The path *count* itself stays small —
the defect is that the recursion has **no per-(callee, judge-summary)
memo**: the same machine at the same abstract point is re-checked once
per sibling path arriving at its call, and judges differ only by
capture-occurrence numbering, so a naive structural key would still miss
unless the occurrence stamps are normalized out of the key.

`normal_completion_paths` already rejoins outcome judges whose paths
added no effects (`has_no_new_effects_or_guarantees_since` →
`rejoin_pure_outcomes`, structural_judgment.rs:750, which intersects
`stored_values` per coordinate). Arithmetic writes always add effects, so
that merge never fires here — the ~8 sibling paths per level recurse 8
separate suffix checks instead of one joined check.

N=24: installs counter shows only ~64 scoped-binding installs; value
transport is NOT the cost.

## False leads (measured, rejected)

- Value-tree depth: stored `ScopedArithmeticValue`s stay < 100 nodes; the
  `Box`→`Arc` sharing refactor built clean but moved nothing — reverted.
- `install_scoped_bindings` re-normalization: ~64 calls at the knee.
- Guard width: one conjunct measured *slower* than two (28.7s vs 18.8s at
  N=64 raw) — conjunct count is not the driver.
- `capture_admitted_integer_remainder` engine clone (`%` probes): adds
  cost but bounded-vs-raw curves are similar; not the dominant term.
- "Exponential" reading of the wall-time table: disproven by
  OMEGA_PATH_COUNT — event counts are polynomial; the knee is swap.

## Next step

Factor the transition-path traversal: `05.d.3.b.flow_fixed_point` already
proves the dataflow-fixpoint pattern this stage needs (716ms for the same
program). Two complementary repairs:

1. **Join sibling paths at callee entry.** Extend the `rejoin_pure_outcomes`
   merge to paths whose effects differ only in occurrence-numbered
   captured terms — i.e., widen `stored_values`/`normal_results` at
   coordinates with differing `current` terms (the intersect loop at
   structural_judgment.rs:771+ already reconciles matched entries; the
   missing piece is a widened term for unmatched ones) — or memoize
   `recognize_structural_state_leaves` on (state symbol,
   occurrence-normalized environment).
2. **Cap the judge's carried state.** The per-event cost is proportional
   to accumulated write facts; a summary/demand view of `walk.environment`
   at callee entry would linearize what each recursion must clone.

Probe N in {26, 28} reproduces the cliff in < 60 s without hitting swap;
`OMEGA_PATH_COUNT=1` counts re-traversals directly.

## Blocks

`samples/gui/windowed_calculator` checking on every host — the GUI
provider-selection acceptance under PROVIDER-SELECTION-AFTER-TERMINAL
cannot reach `run` on any target until checking completes. The same
order-of-growth bounds all deeply-nested samples.

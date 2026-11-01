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

The dedup surface is measured, not guessed (`OMEGA_PATH_COUNT=1` also
prints `[envhash]` env/judge fingerprints at each `bind_invocation`):

- **93% of callee entries share one `target_environment`**
  (13,104/14,132 at N=26): sibling paths bind byte-identical arguments.
- **Every captured-judge fingerprint differs**: judges carry
  `capture_sequence`/`storage_write_sequence` counters and
  `CapturedValue.occurrence` stamps that are pure renaming differences —
  semantically equal states never compare equal.

The fix is an **occurrence-canonical memo** on the state-leaf check:
normalize `CapturedValue` occurrence numbers to first-seen order over the
judge's effect vectors, key `recognize_structural_state_leaves` on
(state symbol, `target_environment`, normalized judge), and rebase the
returned leaves' occurrences per caller (or prove the check
renaming-invariant and share them). `05.d.3.b.flow_fixed_point` already
proves the summary/fixpoint pattern (716ms for the same program); the
simpler `rejoin_pure_outcomes` extension is unsound here because the
paths carry real ordered writes, only stamp-equivalent.

Probe N in {26, 28} reproduces the cliff in < 60 s without hitting swap;
`OMEGA_PATH_COUNT=1` counts re-traversals directly.

## Traversal census (2026-11-03) — the memo target moved

`[walk]` instrumentation at the `recognize_structural_case_arms` /
`recognize_structural_state_prefix` entries resolves where the events
actually come from. At N=26 there are **1,899 obligation traversals**
(1,521 `case_arms`, 378 `prefix`), and the earlier per-caller quadratic
`4·T(n−k)` decomposes exactly:

- **Every site is walked from every ancestor root.** Each machine
  `mk.entry.{0,1}` site (the two `%` writes' obligation sites) is the
  target of a traversal rooted at *each* of `m0..mk` — ~2 walks per
  (root, site) pair (the runtime-prefix walk plus the private-arrivals
  coordinator). Θ(N²) root×site pairs × Θ(N) nested-call events per
  walk = the measured Θ(N³) total.
- **The pairs are semantically distinct.** A walk rooted at `m0` reaches
  `m9.entry` under m0's accumulated path context; the `m1`-rooted walk
  reaches it under a different prefix context. The site judges differ
  legitimately — cross-traversal key dedup has nothing to hit on.
- **Judge-scope memoization measured zero.** An occurrence-canonical
  memo keyed on (state, env, canonical judge, path, hypotheses,
  citations) was built inside `recognize_structural_state_leaves`
  (worktree `psat-fix`): 13,953 lookups, **0 hits** — 13,252 of them
  against empty maps because ~905 distinct `StructuralJudge` memo
  families exist (one per traversal). Within one traversal the
  identical-call multiplicity is already bounded; the redundancy lives
  *between* traversals, where the contexts legitimately differ.

Consequence: the repair is **demand-batched composite walks**, not
memoization. One walk rooted at `mk` already emits prefix events at
every descendant site — but each site's obligation drives its own walk
(~25 sites × ~2 kinds per root). Attaching *all* site visitors for one
root to a single traversal (`observe_runtime_state_prefixes` already
implements the multi-visitor mechanics for `numeric_observations`)
collapses walks per root from Θ(sites) to Θ(1): total work Θ(N²·1)
instead of Θ(N³). The open problem is demand laziness: drivers
(`operand_fits`, `call_requirements`, `stored_predicates`,
`collection_bounds`, …) create per-site visitors synchronously and read
verdicts immediately, so batching needs a per-(root, state) preparation
step that enumerates the demand set up front — all sites are
enumerable `(state, statement_index, in_transition_target)` positions
in the typed trees — then serves driver queries from the completed
batch. `'static` verdict caching in `CheckingMemos` (which stores no
`'program` borrows) is only sound per (driver-tag, machine, state,
site, request-context) and yields only the ~2× runtime/private split;
the composite walk is the structural fix.

## Driver attribution + trial cap (2026-11-04)

Backtrace attribution on `[walk]` resolves the demand owners at N=26
(1,899 walks): `operand_fits` 1,510 (79%), `default_domains::
assignment_windows` 260 (constant library obligations), `numeric_
observations` 38, `validate_machine_contract_entailment` 37,
`call_requirements` 31, `completed_observations` 10,
`default_domains::walk_state` 9, `proof_edges` 3.

Inside `observe_private_arrivals` the dominant multiplier was the
`caller_frontiers` trial loop: each obligation ran O(depth) single-root
trial walks nearest→farthest, and on this shape **all 1,300 trials
failed** (`outcome=true positive=false`). Trials are pure
positive-proof shortcuts — a failed trial discards its cloned
requirement, so skipping trials never changes the verdict; the main
frontier is authoritative. Capping trials to the most-expanded cut
(`caller_frontiers.iter().rev().take(1)`, deepest caller context first)
measured 1,899 → **699 walks** (−63%) and trials 1,300 → 100 at N=26.

## Memory attribution — why m32 still dies (2026-11-04)

`take(1)` fixed walk count, not bytes. m32 (N=32) reaches ~11.5 GB RSS
and is OOM-killed; the 11.5 GB plateau is the host commit ceiling, not
the demand curve (the same plateau appeared at N=40 earlier). Field
sizes of `StructuralJudge` sampled mid-run are tiny (`stored=2`,
`bodies=depth`, every other fact vector 0) — the memory is **copy
churn, not retention**: every path event clones/normalizes a judge
whose `Arc<Vec<StructuralTerm>>` fields are `Arc::make_mut`-copied on
write, so per-event bytes ≈ Θ(d) entries × Θ(d) term size = Θ(d²) with
Θ(N³) events ⇒ Θ(N⁵) allocation churn and Θ(N²–N³) live leaves
(`leaves.extend(completed)` unions each callee's outcomes upward).
Leaf `StructuralCaseArm`s each retain a full judge plus owned
environments/citations.

Also corrected: the earlier "m26 in 0.15 s" was a
`compiled-review-cache` hit, not a real check — the doc's cold-cache
warning applied; cold N=26 remains ~seconds. N=26 peak RSS ≈ 235 MB.

Consequence for the fix: batching walks removes the Θ(N²) walk
multiplier, but each surviving walk still pays Θ(d²) copy per event.
The composite repair therefore also wants leaf/streaming delivery
(consuming verdicts without retaining every sibling arm) and/or
persistent-structure judge fields — otherwise the walk-count win is
eaten by the per-event byte multiplier at larger N.

## Composite-batch implementation notes (2026-11-04)

Precise semantics worked out against the code; preserved for the
implementing session.

**Where the depth lives.** `observe_runtime_state_prefix` roots at the
site's own `(machine, state)` — shallow. The deep walks are
`observe_frontier`'s `recognize_structural_case_arms` per frontier
root: `ArrivalRequirement::stops_at_machine` halts descent at machines
`∈ boundaries ∧ ≠ current_root` *and* at machines outside the
obligation's `closed.bodies` cone, but a root's walk still traverses
everything up to the first boundary — under `mk`'s frontier the
`m0`-leg reaches `m_{k-1}` (O(k) deep). Stopping at a boundary still
walks that machine's own statements (that is where
`entered_body(caller→entry)` observes the expected arrival).

**What is shared per root walk.** The judge is
`from_runtime_state_requires(root)` — obligation-independent, so one
traversal can serve every obligation whose frontier lists that root.
Per-obligation state is confined to: the wrapped site-requirement
(`Request`), `entry` (the obligation machine's entry), `boundaries`
(its frontier's root set), `closed` (its arrivals closure), and the
`observed` arrival set. A `BatchedArrivalObserver` wraps N members;
`stops_at_machine` = all members stop (walk goes to the deepest needed
depth), `is_complete` = all complete, and every event is delivered to
each member **masked by whether that member's own walk would have
reached the event's machine** (see below). `site()` is only consumed
per member — `prefix_site_is_reachable` must run per member before
registration.

**No masking needed — streams are identical per root.** Verified
against the code: `stops_at_machine` is consulted *only* inside
`prefix_site_is_reachable` (the per-root precheck deciding whether a
root's walk counts at all) — the traversal never prunes descent.
Every non-skipped root walk traverses the entire nested cone and
delivers every event unconditionally, so all members of a (root,
case_facts_only) batch receive byte-identical streams: broadcast each
event to all members, gating per-member delivery on `!member.
is_complete()`. `prefix_site_is_reachable` runs per member before
registration; unreachable members are simply absent from that root's
batch, exactly as their solo `continue` behaved. (`entered_body` args
are *state* symbols — caller state + callee entry state — resolve
machines via `machine_state_by_symbol` when needed.)

**Descent eligibility is per-member.** At 2008-2018 the nested
walker *skips* a transition descent when `site().state ==
state.symbol` (site is here, a jump leaves the invocation) or
`!prefix_site_is_reachable(target_state, observer)`. `call_entries::
observe_statement` does the same at 37 (site already on `path` or in
this state → early return) and 52 (site unreachable via the callee →
`continue`). `entered_body` must be delivered **only to members whose
eligibility test passes for that descent** — over-delivered call-entry
events change inner requirements' verdicts. Per-descent, build the
eligible member subset (`v.site().state != state.symbol && !
path.contains(&v.site().state) && prefix_site_is_reachable(..., v)`),
send `entered_body` to that subset, and recurse with the eligible
subset only.

**Union-walk over-diagnosis edge (accept + document).** A batched
walk descends wherever *any* member is eligible, so its arm set is the
union of members' solo cones — identical per-member leaves where they
overlap (same judge/frames), but diagnostics/`?`-aborts from a
member's extra subtree can mark the shared walk unrecognized where
that member's own solo walk skipped it. This is an incompleteness
(reject-direction) risk only — never a wrong-accept — and is the
right trade versus not terminating; corpus CI adjudicates.

**Signature conversion.** The chain is `recognize_structural_case_
arms` (entry, `Option<&mut dyn>`) → `recognize_structural_state_
leaves` → `recognize_structural_state_leaves_from` (1184,
`requirement: &mut Option<&mut dyn StructuralPrefixVisitor>`) →
`call_entries::observe_statement` (33) → recursion. Convert the inner
three to `visitors: &mut [&mut dyn StructuralPrefixVisitor]`; keep the
public Option entry and internally wrap with `map(std::slice::
from_mut).unwrap_or(&mut [])`, adding a `…_batched(visitors)` entry
for `observe_frontier`. Touchpoints inside `…_from`: is_complete
early-exit → `all complete` (1196); observation_position → max
position over members sited in this state (1235); visit gates →
per-member site match at 1271/1725/1779/1963; `is_some_and(site…)`
prunes → `iter().any` at 1442/1468/1950; `is_some()` gates →
`!visitors.is_empty()` at 1289/1340/1740/1999/2152/2166/2198;
entered_body → eligible subset (2105); descend → any-eligible
(2008).

**The real blocker: site() drives the nested walker.** Inside
`recognize_structural_state_leaves_from` (structural_case_arms.rs
1168+), `requirement.site()` both *truncates* each entered state's
statement loop at the site position (1236/1265 `break`) and fires
`visit` only at the exact site statement (1272+; also 1443, 1469,
1730, 1786, 1968). A single `site()` cannot express N members, so
batching requires a **plural nested walker**: thread
`&mut [&mut dyn StructuralPrefixVisitor]` through
`recognize_structural_state_leaves` / `…_from` /
`recognize_structural_case_arms` / `call_entries`, replacing each
`requirement.site()` gate with per-visitor loops that take the max
needed position across members whose `site().state == state.symbol`
and fire each member's `visit` at its own statement — exactly the
mechanics `recognize_structural_state_prefixes` (544-1125) already
implements for the same-state case (`any_visitor_at_transition`,
per-visitor `valid[]`, per-visitor `is_complete` skips). Then
`observe_frontier` grows a batched twin holding
`Vec<ArrivalRequirement>` members per root walk, and
`observe_private_arrivals_batch(program, machine, &mut [&mut Request],
case_facts_only) -> Vec<Option<bool>>` shares the arrivals/frontier
computation across a machine's obligations.

**Same-machine shortcut.** Every obligation on `mk` shares `entry`,
`boundaries`, `closed`, `roots`, `expected` — members differ *only* in
the wrapped `Request`. Batching per machine is a ~sites-per-machine
constant cut; the asymptotic win is the cross-machine root pool
(`m0` is a root in *every* nested machine's frontier, so pooling it
serves N obligations' m0-legs in one O(N) walk).

**Driver plumbing.** Demand laziness is the real blocker —
`observe_operand_fit_at_occurrence` builds one `OperandFit` visitor,
waits for the verdict, returns. Two workable shapes:

- *Upfront enumeration*: `exact_binary_formation_at_occurrence` is a
  pure function of `(machine, state, expression)`; the exact-domain
  gate inside it is itself the occurrence predicate, so a syntactic
  pre-pass over `state.statement_nodes` expressions reproduces the
  demand set. Collect requests per machine (or globally), group
  private-arrivals requirements by frontier root, run one composite
  walk per root, then answer per-occurrence queries from the verdict
  map. Runtime-prefix legs batch per `(machine, state)` through the
  existing plural `observe_runtime_state_prefixes`.
- *First-occurrence machine batch*: when `mk`'s first operand-fit
  query arrives, enumerate `mk`'s remaining occurrences eagerly and
  run one same-machine batch; later queries read the stored verdicts.
  Same constant cut as the shortcut above, contained to
  `operand_fits.rs` + a slice-based `observe_private_arrivals_batch`.

**Soundness edge to keep.** `case_facts_only` and proof-machine
classification alter the judge (`restrict_to_case_facts`,
`from_requires` vs `from_runtime_state_requires`) — batch members must
group by `(root_symbol, case_facts_only)`, never mix. Truncated runs
(a member's `is_complete` early-out does not truncate the shared
walk — only `all complete` does) are handled by the completion
semantics above.

## Verified results — merged batching (2026-11-05)

Landed: `take(1)` frontier-trial cap (#1070) + operand-fit
demand-batching (#1090, plural nested walker + per-machine population
memo + `observe_private_arrivals_batch`). Post-merge census adds
`[walkb]` counters on both batched entries (members > 1 only).

| probe | walks `[walk]`+`[walkb]` | members served | wall | verdict |
|-------|--------------------------|----------------|------|---------|
| m26   | 349+126 = 475            | 761            | ~1 s | fails (m21-m25 range proofs) |
| m32   | 348+144 = 492            | 820            | ~1 s | fails (m21-m31) — was OOM at 2m46 / 11.5 GB |
| m48   | 541                      | —              | ~1 s | fails |
| n128-b| 782                      | —              | ~3 s | fails |
| m32-const (clean) | 470            | —              | ~3 s | **passes** |

The Θ(N³) walk structure is collapsed: demands grow ~linearly in N and
are served by ~O(N) batched walks. `window_app` --check now completes
in **17 s** (previously unbounded); `windowed_calculator` still exceeds
600 s — remaining walks are `default_domains::assignment_windows` (67%)
and `walk_state` (28%), per-site consumers of
`completed_value_case_at_occurrence` → `observe_runtime_state_prefix`
(singular). Same batch shape as operand_fits; that consumer is the next
plumbing target.

**Identical-verdict evidence:** pre-merge `bd87af7` and post-merge
m26/m32 produce byte-identical error sets — the batch preserves
semantics on this shape. The m26/m32 range-proof rejections are a
**prior main regression** (deep-machine `%`/`+` operands on unbounded
`u64` now unprovable), independent of the batch work — flagged, not
fixed here.

**Session trap recorded:** `omega-remote`'s default `Remote` slot is
shared across sessions — a foreign sync reset `~/Omega-Remote` to an
unrelated head mid-session, silently building/running stale code.
Also, each local `exec` spawn starts at the primary checkout's cwd,
whose pinned anchor can lag main — `omega-remote` resolves HEAD/TOP
from cwd, so calls must `cd` to the intended worktree first and
prefer a private `--slot`. Symptom: 0 `[walk]` lines + ancient `git
log` on the remote worktree.

## Blocks

`samples/gui/windowed_calculator` checking on every host — the GUI
provider-selection acceptance under PROVIDER-SELECTION-AFTER-TERMINAL
cannot reach `run` on any target until checking completes. The same
order-of-growth bounds all deeply-nested samples.

## windowed_calculator frontier (2026-11-05) — in-traversal cycle blowup

Completed-case batching (#1122 pending) collapsed the walk count:
370 singular site walks -> 25 singular + 58 batched (431 occurrence
members). The surviving wall is *inside one walk* on
`display_digit_slot`'s `Handle` state - an exponential/divergent
prefix traversal, not a count problem.

Evidence (OMEGA_PATH_COUNT census, killed at 39 CPU-min):
- `[envhash]` fingerprints recur exactly: `env=f5509a3b9745a2bd` at
  depth=5, depth=9, depth=21; `bodies=N` grows 1/depth while the
  semantic-value fingerprint holds - identical environments are
  re-entered along the cycle.
- The `path` cycle cut (`structural_case_arms.rs:1257`) only fires on
  `start_index == 0` re-entry; mid-state re-entries ("a cycle back is
  a new arrival", ~:2194) recurse per `bind_invocation` frame with no
  dedup, so a cyclic machine topology (wcalc's ~40-state pump/input
  loops) spins the depth without bound.
- Judge fingerprints always differ (capture/storage sequence
  counters) but the env fingerprint is stable - an arrival memo keyed
  on (target_state, env fingerprint, entry kind) would hit; the
  semantic_value hash already excludes counters.

Next repair: canonicalized (state, env, entry-kind) arrival dedup on
the recursive descent in `recognize_structural_state_leaves` - a
second visit along the same ancestry adds no new provable arrival;
determinism makes its subtree identical to the first visit's. Entry
kind (fresh vs mid-state index) must be in the key since mid-state
entries owe different contracts.

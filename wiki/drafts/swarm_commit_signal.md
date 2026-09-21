# Swarm commit signal-to-noise

Evidence review of why the last ~1,000 commits read as low signal-to-noise,
checked against the published swarm-orchestration record (Anthropic, Cursor,
Cognition). Written 2026-09-20 against window
`519470a5be..7fb7cbf216`. Remove when the board/landing workflow changes make
its numbers stale, or when a later measurement supersedes it. Not a policy
document; recommendations are candidate experiments, not decisions.

## The measured window

1,000 commits over 1d 19h 43m (~23 commits/hour sustained, peak 97/hour).
Two author identities — Jarod (648 commits, 54 active hours) and Zachary
(351 commits, 34 active hours) — both fronts for machine swarm slots, not a
headcount. Median inter-commit gap 1.4 minutes; 55% of commits land within
2 minutes of their predecessor. That cadence is the landing queue's
three-minute serialized lease (`tools/landing.md`), not organic work rate.

Content classification by touched paths (a commit can hit several classes):

| Class | Commits | Note |
| --- | --- | --- |
| code (`omega-rust/`, `source/`) | 674 | includes 250 board+code mixed landings |
| board (`TASKS*.md`, `OWNER_QUESTIONS.md`, waves) | 533 | **218 commits are board-only** |
| tests | 205 | |
| docs | 78 | tooling 36, build 19 |

Commit sizes are not the problem: median 205 lines touched, p75 = 569, only
65 commits under 10 lines. This is not Cursor's flat-swarm failure mode of
small safe edits. Subjects naming revert/fixup/wip/repair: 6 of 1,000 — the
noise is not rework loops either.

The noise is structural:

- **Coordination bookkeeping is serialized into product history.** 454
  commits touch `TASKS.md`; 22% of all commits are *only* board bookkeeping
  ("board: X records discharge…", "swarm w9: 54 mined board items"). Every
  landed slice pays ~1 product commit + ~1 board commit, so the log doubles
  in length without doubling in content.
- **Work lands as fragments of items that never close.** Wave-6 outcomes:
  5 of 7 sessions landed, `item_closed: false` on all five. Items return in
  later waves (the launcher README's "repeated rediscovery cost"), and each
  return generates its own commit+b board entries. 25 distinct board items
  named across 218 board-only commits; the median item is re-witnessed,
  re-sliced, and re-recorded rather than closed.
- **Two shared hotspots take cross-machine churn.** `canary_suite.rs`
  touched 57 times, split 29/28 across the two machines;
  `terminal-verifier/.../sites.rs` 26 times (24/2). The umbrella test
  registration file is a Cursor-style "megafile": every agent adds a small
  piece and nobody owns its size or its conflicts.

## What the published record says

### Anthropic agent teams — flat works only with a perfect verifier

Carlini ran 16 Opus 4.6 instances on a shared bare repo for ~2 weeks, ~2,000
sessions, $20k → 100k-line Rust C compiler that builds Linux 6.9. Coordination
was `current_tasks/*.txt` lock files in git plus a Ralph loop; **no
orchestrator**. What carried it: a near-perfect test/verifier harness (GCC as
online oracle to partition the one-big-task kernel build), per-agent
orientation aids (READMEs, progress files), context-pollution control in tool
output, and specialized roles (dedup, perf, docs). Where it broke: agents
picking "the next most obvious problem" all converged on the same kernel bug
and overwrote each other until the GCC-oracle harness made work partitionable;
near the end, new features kept breaking existing functionality until CI
enforcement was added. Even then the compiler stayed "nowhere near" expert
quality and resisted further improvement.
<https://www.anthropic.com/engineering/building-c-compiler>

### Cursor scaling-agents — flat coordination fails; planner/worker tree scales

Wilson Lin's report on hundreds of agents/~1M LOC/~1 week (FastRender).
Equal-status agents self-coordinating through a shared file + locks: locks
held too long or never released, 20 agents → throughput of 2–3, agents turned
risk-averse (small safe edits, nobody owns the hard spine), "work churned for
long periods without progress." Optimistic concurrency removed the lock
brittleness but not the accountability vacuum. The fix: **planners**
(recursive, parallel, own task creation) / **workers** (one task, no
coordination, push when done) / a **judge** per cycle. An integrator role was
a net-negative bottleneck — workers resolve conflicts themselves. Model per
role; prompts mattered more than harness; "the right amount of structure is
somewhere in the middle."
<https://cursor.com/blog/scaling-agents>

### Cursor swarm economics — commit rate is a noise metric, not a signal one

The follow-up rebuilt SQLite-from-docs (835-page spec, held-out sqllogictest
grade) under old vs new harness, same models and budget:

| Metric | Old (flat-ish) | New (tree) |
| --- | --- | --- |
| Commits, first 2h (Grok 4.5) | 68,000 | ~1,000 |
| Merge conflicts | >70,000 and accelerating | <1,000 over 4h |
| Hottest-file conflicts | 7,771, touched by 1,173 agents | 47 |
| Crate count | 54 (three duplicate SQL packages) | 9, stable early |
| LOC for same 100% grade | 64,305 | 9,908 |
| Suite grade at 4h | 11–77% | 73–85%, then 100% |

Named failure modes and fixes: split-brain design (planners must decide, not
delegate, and no two subtrees decide the same question); planner contention
(shared design docs + compile-checked references + reconciler, because merges
can't fix two pictures of reality); merge conflicts (neutral third-party
resolver, like a merge queue); megafiles (workers flag, commits blocked,
outside agent decomposes); ossification (licensed intentional breakage with a
comment the compiler propagates); review (stacked decorrelated lenses — cheap
relative to audited work); stigmergy/Field Guide (agent-curated shared context
with a line budget). Frontier-planner/cheap-worker mixes held quality while
the worker fleet dropped from $9,373 to $411.
<https://cursor.com/blog/agent-swarm-model-economics>

### Cognition — the dissent that the other two actually obey

"Don't Build Multi-Agents" (Yan, Jun 2025): parallel write-agents violate two
principles — share full context/traces, and actions carry implicit decisions
that conflict when made concurrently. Its prescription (single linear agent +
a compression model for long horizons) is stricter than what Cursor shipped,
but Cursor's winning design concedes the point: decisions are centralized in
planners/design docs so workers never make conflicting implicit decisions,
and workers' narrow scopes bound the blast radius. Anthropic's flat team
similarly survived only where work partitioned so cleanly that implicit
decisions couldn't collide — and broke exactly where they did (the kernel
task).
<https://cognition.ai/blog/dont-build-multi-agents>

## Where Omega sits on that map

Omega's swarm is already closer to Cursor's working shape than to the failed
flat designs: a coordinator partitions via manifest (planner-ish), workers run
one bounded board item, the claims registry is an advisory fence (correctly
not a lock), and the landing queue serializes publication (Cursor's neutral
merge-resolver role, at git-push granularity). The measured noise comes from
specific gaps, not from the topology:

1. **The board lives in the product channel.** Cursor moved coordination to
   design docs with compile-checked references and a reconciler; Anthropic's
   lock files were at least disposable. Omega commits board state to `main`
   ~1.2 times per landed commit. Every `TASKS.md` touch is also a
   rebase-conflict surface the landing queue then serializes around.
2. **Board truth lags landed reality**, so partitioning consumes stale input:
   the README's Sept screen found stale/duplicate assignments in 4 of 11
   reviewed flags, and wave-6 lost sessions to claims on paths that weren't
   the real seam ("revise owning_paths next wave"). Cursor's version: two
   pictures of reality that merge tooling can't fix.
3. **Items don't close** (0/5 in wave-6), so the swarm re-attempts the same
   items across waves — the repeated-rediscovery cost the README predicts.
   Commit count per closed item is the honest signal metric; raw commit rate
   is the metric Cursor's data discredits.
4. **`canary_suite.rs` is an unowned megafile**: 57 touches/44h across both
   machines. Cursor's fix was a flag-and-decompose protocol; Omega's could be
   per-group registration files so umbrella edits stop contending.

## Candidate experiments

Ordered by expected signal gain per unit of disruption; each is reversible
and measurable against this window's numbers:

- **Split bookkeeping from product history.** Batch board updates per landing
  (one commit), or land board state on a coordination ref and mirror a digest
  to `main` daily. Success measure: board-only commit share → <5%; landing
  conflicts on `TASKS.md` → ~0.
- **Enforce the existing freshness rule.** `plan` already probes owning-path
  freshness; make stale-item manifesting fail rather than warn (README §
  "still unimplemented"). Success measure: fewer `blocked`/mispartitioned
  outcomes like wave-6's two lost sessions.
- **Track commits-per-closed-item per wave** in `*.outcomes.json` (schema
  already has `item_closed`, `acus_consumed`; add commit count) and treat
  closure, not commits, as the wave's unit of output.
- **Decompose `canary_suite.rs` registration** to per-group files, or gate
  umbrella edits behind one owner per wave. Success measure: cross-machine
  touch count on the umbrella drops; landing rebases on it approach zero.
- **A standing review lens.** Cursor's strongest quality lever was cheap
  decorrelated review of worker output; Omega's `retrospect` skill is the
  batch version. A per-wave review pass over landed commits (not board
  commits) is the continuous analog.

## What was not measured

Session-level ACU/token cost (cloud receipts had `acus_consumed: null`),
per-item commit counts (item names aren't consistently in subjects), and
whether board commits ever block landings in practice (the queue hides the
cost in its 3-minute lease). The next wave's outcomes file can close the
first and third cheaply.

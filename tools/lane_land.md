# Land a prepared lane from one tool

`tools/lane_land.py` takes a *prepared* lane — a worktree holding its lane
commit(s) on top of some `origin/main` ancestor, with evidence recorded in the
commit body or a prior chain log — through the whole lease in one build-slot
hold: fetch, rebase, optional `mbx nextest`, `landing.py verify`, enqueue,
claim, rebase onto the granted base whenever it moved, re-verify, claims note,
publish, and the remote ancestor check.

```sh
python3 tools/build_slot.py --slots 2 --timeout 0 -- \
    python3 tools/lane_land.py <worktree> --note-ticket <ticket> --nextest <crate>
```

## Why one tool instead of one script per lane

During the 2026-10-24 wave, roughly thirty per-lane chain scripts re-spelled
this lease and each acquisition was also re-burning the same defects: poolean
movement guards that released healthy attempts whenever any unrelated `.rs`
file moved (they fired for most lanes once the fleet landed every few
minutes); landing tickets passed to the claims registry's `note` (which
aborts the chain before publish); `--provenance` passed to `enqueue` instead
of `publish` (which refuses the ticket). Ready lanes with green evidence and
no owner then waited 8-16 h each for a slot and often gave up unlanded.

Folding the lease into this one tool (driven by one loop, no per-lane agent)
changed ready→landed from hours-to-never to 11 minutes on the first folded
lane, landed two lanes whose per-lane chains had failed all day, and removed
the per-lane script sprawl entirely. The measured lesson is in
[the swarm host setup](../../.agents/skills/local-swarm/host-setup.md):
when queue depth is a multiple of the pool's slots, fold — one loop and one
tool — and spend agents on parallel *thinking*, not parallel *waiting*.

## The two behaviours that matter

- **Rebase-always, scoped movement.** When main moves between measurement and
  the granted base, the tool rebases onto the granted base and re-verifies.
  A conflicting rebase is the only genuine stale-evidence case, and it
  fails loudly (`INLEASE_REBASE_CONFLICT`, exit 36) with the claim released.
  Blanket "any Rust moved" guards release nearly every attempt under a busy
  fleet; this tool never fires on unrelated movement because the rebase and
  the re-verify restore the evidence either way.
- **No dangling claims.** Every failure path after a granted claim releases
  it before exiting (codes 36-39). Failure before the claim never touched the
  queue. A successful run writes `build/lane-land/LANDED` with the candidate.

## When to use it

Any prepared lane without a live owner. Run it inside one
`build_slot.py --timeout 0` hold — nested holds short-circuit through
`OMEGA_BUILD_SLOT_HELD`, so the lease's internal verifies never re-queue
([build slots](build_slot.md#one-hold-for-a-whole-chain)). One loop may drive
many lanes sequentially; each lane costs one pool acquisition total.
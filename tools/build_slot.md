# Build slots

Agent count and build count are separate resources. One `mbx build -p omega`
drives rustc to roughly 5.4 cores on a 10-core host, so two concurrent builds
saturate the machine; a third adds contention rather than throughput, slowing
every build, starving the landing queue behind them, and leaving agents that
are not compiling unable to make progress.

`tools/build_slot.py` caps only the build resource. Wrap an expensive command:

```sh
python3 tools/build_slot.py -- mbx build -p omega
python3 tools/build_slot.py --slots 2 --timeout 3600 -- mbx nextest run -p psi
```

The standing configuration on this host is **8 worktrees, 8 subagents,
2 build slots**. The pools are independent: eight agents may edit, author
fixtures and run programs at once while two build at full speed.

Waiting is bounded and reported on stderr, so a caller can tell "queued" from
"hung" (`build_slot: waiting for one of 2 slot(s); held by [...]`). A slot
whose owning process has died is reclaimed instead of wedging the pool, and a
timeout exits 75 rather than starting the build.

Slots live under the checkout's ignored `build/build-slots/`, keyed by the
common git directory, so all worktrees of one checkout share a pool while
unrelated checkouts do not contend.

Waiting is a race among pollers, not a queue: every waiter wakes on its own
cadence and the first to see a freed slot takes it, so waiting longest grants
nothing. On 2026-10-23, amid a fifty-lane wave, single chain acquisitions
waited 3,039-5,282 seconds (up to 88 minutes) for one of the two slots —
longer than any per-step re-acquisition pattern can survive inside a
`--timeout`.

## One hold for a whole chain

Cover local checks, tests, corpus runs and builds with a single acquisition:

```sh
python3 tools/build_slot.py --slots 2 --timeout 5400 -- bash chain.sh
```

`hold()` exports `OMEGA_BUILD_SLOT_HELD=1` into everything it wraps, and the
flag is inherited: nested `build_slot.py` and `corpus_gate.py` runner builds
short-circuit instead of queueing again. Run the local steps sequentially.
Release the slot before pushing a PR or waiting for GitHub's merge queue;
[landing](landing.md) runs its integration check on GitHub, not in this pool.
Docs-only changes need no compiler-build hold.

## Diagnostics don't queue with chains

Fixing a compile error or a red test is a minutes-scale check-edit-check loop.
Run those single-crate checks in a separate named pool so they neither wait
behind nor starve the chain slots:

```sh
python3 tools/build_slot.py --pool diag --slots 1 -- mbx check -p <crate>
```

The diag pool keeps its own slot file (`build/diag-slots/slot-0`) and its own
short-circuit variable (`OMEGA_BUILD_SLOT_HELD_DIAG`), so it never contends
with — or gets silenced by — the two chain slots. Budget one extra rustc
process: keep diag commands short and single-crate, and under memory
pressure, drop diag work before chain builds.

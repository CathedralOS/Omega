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

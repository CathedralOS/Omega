# Squalr geometry — native host legs

Record of the per-host native-execution legs for the Squalr geometry
application's parity acceptance. Source of truth for the acceptance itself is
the app-board row `GEOMETRY-PARITY` in `samples/apps/squalr/TASKS.md`
(submodule pin `5b0307c352`); Omega-side tracking lives under
`SQUALR-GEOMETRY-PARITY` and the `GEOMETRY-*` sibling rows in TASKS.md.

## Acceptance leg

Inside the pinned `samples/apps/squalr` checkout:

```
python tools/verify.py native --timeout 600 --omega <executable>
```

which runs `omega run --keep squalr-tests/main.omg` and requires exit 0 with
`Squalr geometry: PASS` across the 12 authored geometry checks. The run-based
acceptance cannot be satisfied by a cross-target emit leg (e.g. a Linux
`--target windows_x86_64` build) — the evidence must come from a native run on
the named host.

## Host ledger

| Host | Status | Evidence |
| --- | --- | --- |
| macOS ARM64 | PASS | Original 12-check evidence: Omega + std `87d8b227`, `Squalr geometry: PASS`, ~167.2s end-to-end. |
| Linux x86-64 | PASS | w9 z57 witness at `d82697ffca`: release-profile `omega`, Python 3.10 + `tomli` shim, `RUST_MIN_STACK=67108864` — `Squalr geometry: PASS`, exit 0, 173.2s end-to-end. |
| Windows x86-64 | UNRECORDED | "Windows was not run" — no Windows development host exists in the swarm environment; owned by GEOMETRY-WINDOWS-VALIDATION / -LEG / -REVALIDATION. |
| Linux ARM64 | UNRECORDED | No run recorded on any row. |

## Running a new host leg

Fresh-checkout ceremony (required before any run — the tracked
`squalr-tests/omega.lock` is bound to the committing checkout's canonical
`ExternalLocal` path and rejects other checkouts with "fresh source key or
immutable content differs"):

1. Copy the app tree to scratch (the fenced `samples/apps/squalr` path rotates
   between wholesale dir claims — check `tools/claims.py status` first).
2. Remove the stale `squalr-tests/omega.lock`.
3. `omega update --project squalr-tests`, accept the six pending decisions in
   `build/package-manager/review-<target>.txt`, then `omega update --resume`.
4. Run the acceptance command above.

Environment notes:

- Use a release-profile `omega`. A debug-profile build does not work for this
  leg: the per-checkout `omega update` evaluation took ~62 min and the compile
  leg exceeded the 30-min timeout; the release build compiles and runs in
  ~3 min.
- `verify.py` requires Python >= 3.11 (`tomllib`); on 3.10 hosts supply a
  `tomli`-backed `tomllib` shim on `PYTHONPATH`.
- `RUST_MIN_STACK=67108864` is needed on the Linux host.

## Ownership

Residual host legs are board-owned, not doc work:

- Windows x86-64: GEOMETRY-WINDOWS-VALIDATION cluster (host-gated — needs a
  Windows development host).
- Linux ARM64: unclaimed; any runner can record it with the ceremony above.

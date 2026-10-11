# Validation contract and provenance

This source snapshot is frozen before its final package review and validation.
Actual observations are generated in ignored `build/validation/` and included
in the independent review bundle. This document specifies acceptance; generated
logs, not prospective claims in this file, establish the result.

Initial base: `678203d30209af45e0446530582ea89914fd6180`.
Current-main reproduction and integration base: `33e34fd97e763f4725c6290f0a0d6ea1ec829224`.
Starting checkout `/workspace/Omega`, branch `work`, was clean and is preserved.
Isolated candidate: `/workspace/Omega/.codex/worktrees/add01`, local branch
`add01-force`. No remote worker branch, additional worker or active-worker
algorithm is involved. The existing sensor_min_max source is unchanged between
the starting checkout and the fetched base.

Initial compiler build: `source /workspace/cloud-setup/activate.sh` then
`mbx build --release -p omega`, exit 0, 13m30s. Rust 1.100.0-nightly
(a69a63265, 2026-09-03), mbx 1.21.0, optimized release profile.
Binary SHA-256: `a6e704ba2fb2b4fa6f565dc452953ab3ad0b5d390525c1096f471a2d6f4dd7f4`.
Binary: `/workspace/cloud-setup/cache/mbx/targets/v1/d54cb311dd0c12adea2b03125ae1fd6eb6a961d2f84e7870e8766274501f4132/release/omega`.
Subsequent compiler rebuilds and their exact binary hashes are recorded in the
review bundle. Every invocation selects this checkout's `source/library` via
`OMEGA_LIBRARY_ROOT`; one Rust build and one Omega invocation run at a time.
Host/target: Linux x86-64 / `linux_x86_64`; 5 available CPUs, 17 GiB reported
RAM; `CARGO_BUILD_JOBS=4`. One Rust build and one Omega invocation at a time.
The bundled library is unchanged. Two bounded compiler repairs accompany this
application: preserve fixed-byte view transport without a conflicting reference
carrier; register other reference carriers by their declared referee; and share
exact owned block-home resolution between scalar field and primitive stores.
Complete regression fixtures cover byte/integer state loans and caller-observed
owned-state array/field writes. The unchanged application witnessed both gaps:
first on current main at `main::next`, then at the final parser array store.
Diagnostic-only tracing is removed from the candidate. These are general storage
and control joins; no application name, size or fixture constant selects them.

The ordinary sandbox reports root ownership 65534 and rejects source resolver
cache custody. The approved per-command supported context retains UID/GID 1000
and reports actual `/` ownership 0:0. No ownership or custody guard was changed.
The initial custody rejection and supported context observations are retained.

Application source, parser, renderer, build metadata and reference fixtures are
listed in `SOURCE_SHA256SUMS`; its own hash and the candidate commit are recorded
externally in the review bundle to avoid a self-referential manifest. Package
lock/review state is host-local and excluded from candidate source. Consumers
must perform the documented local package review instead of transplanting locks.

Before any Omega execution, `data/frozen.sha256` froze two streams and exact
stdout oracles. Primary stream: 17 frames, including all runtime diagnostic
classes, oversized-line recovery and unterminated EOF. Variant stream: 31
frames, including mirrored support boundaries, each one-cell corner, maximum
loads and 24 seeded diverse frames. Numerical tolerance is exactly zero;
`data/reference.py` independently uses rational dot products and Fraction.

After local package preparation, from the repository root:

```
PYTHONDONTWRITEBYTECODE=1 python3 samples/cli/independent/force_plate_balance/validate.py target/release/omega
```

The harness runs these six public CLI invocations serially (absolute source and
binary paths are recorded in generated `commands.json`):

1. `omega --check --target linux_x86_64 main.omg`, expected exit 0.
2. `omega run --keep main.omg < data/frames.txt`, exit 0 and exact primary stdout.
3. `omega run --both --keep main.omg < data/frames.txt`, exit 0, exact native
   stdout, and explicit `interp exit: 0` with no DECLINED result.
4. `omega run --keep main.omg < data/variants.txt`, exit 0 and exact variant stdout.
5. `omega run --both --keep main.omg < data/variants.txt`, same checks as step 3.
6. `omega --check --target linux_x86_64 intended_invalid/reject.omg`, expected
   exit 1 and `divisor must be proven nonzero`. This pin matches the stable
   `fail/arithmetic/exact_divisor_unproven` control; unrelated refusal is not success.

Native stdout is compared byte for byte. The repository's Terminal CLI compares
termination status only; its output is not independently exposed or compared by
this harness. Do not describe status agreement as full trace or stdout agreement.
Generated retained native/Terminal artifacts and their hashes are inventoried in
the review bundle. Other hosts are not claimed as executed on this Linux executor.

`python3 data/reference.py` verifies the frozen primary oracle. SHA-256 checks
verify both frozen streams before and after validation. README documents model
limits and physical first-moment/torque conventions.

# Watershed spill analysis (ADD10)

A complete offline synthetic research application in Omega. Feed a 5×5 raster
of single decimal elevation digits on stdin and close input. Spaces, tabs, LF
and CR are allowed. Exactly 25 digits are required; all other bytes, truncated
input and extra heights produce exit 2 and one error line. No worker library or
candidate is imported. Private storage, minimax solver, drainage and decimal
rendering live in this directory. The only dependency is stable bundled std.

The application emits four 5×5 integer grids: `filled` elevations, `outlets`
(catchment labels), `parents` (drainage cell indices) and `volumes` (storage by
outlet index), followed by `total` storage. Indices are row-major 0–24. Each
boundary cell is an outlet to the exterior. Four-neighbor connectivity applies.
Filled elevation is the minimum, over paths to any boundary, of the maximum
original elevation on that path. Linear-scan Dijkstra assigns parents to earlier
settled cells and inherits their boundary outlet. Equal-cost choices use ascending
cell selection and ascending neighbor visitation; only strict improvements replace
parents. This yields an acyclic drainage forest even on flat filled plateaus.
Labels describe this deterministic spill drainage, not geological basin identity.
Volume equals the sum of filled minus original elevation with unit cell area.

The complete forest and all outlet totals are emitted, including zero-storage
outlets. Runtime is bounded by 25 initialization cells, 25 selections, 625 scan
comparisons, 625 neighbor tests and 25 volume contributions. The integer storage
capacity is at most 81 (nine interior cells × maximum depth nine). Whitespace
input can be arbitrarily long and hosted input can wait; the algorithm bound does
not bound input wait or bytes consumed.

## Distinction against existing source

At main `9a0463cc42af5f7178372c169ae55e528fe43878`, the actual
`samples/cli/algorithms/maze_flood/main.omg` computes BFS distance in a 5×5
wall maze. Its edges all have unit distance and it reports a far-corner distance.
It does not use elevations, minimize maximum path height, resolve plateau drainage
or calculate volume. Searches of the current samples tree for micrograph,
watershed, catchment and region labeling found no admitted micrograph application
on this base; no active-worker candidate was used to supply that comparison.
Reachability and generic region labels alone do not deliver this workflow.

At integration main `f3c3255420f4d0eedcf5279fc17d51e7577f1f51`,
`samples/cli/independent/micrograph_regions/domain/regions.omg` is admitted. Its
actual implementation thresholds grayscale intensities, then uses a FIFO queue
to label foreground components with 4- or 8-neighbor connectivity. It reports
region measurements. This application instead computes elevation-dependent
minimum spill heights, assigns drainage to boundary outlets and sums retained
water depths. Its filling and volume oracles remain independent of image
thresholding and connected-component labeling.

## Frozen independent reference

Before the first Omega execution, `reference/oracle.py --freeze` generated the
committed expected stdout and `reference/frozen.json`, containing input/output
SHA-256 and exit oracles. Numerical tolerance is **zero**. Filling is independently
calculated by exhaustive boundary flooding at each integer water elevation 0–9,
without the Omega solver's priority scan. A heap implementation separately specifies
the deterministic tie policy; every parent chain is checked for adjacency,
monotone filled levels, absence of cycles and the claimed boundary outlet.
Per-outlet and global volumes are independently summed from the flood result.
Normal validation checks these frozen bytes and never rewrites them.

Cases include a closed bowl, a lowered spill outlet, two separated basins, a flat
plateau, a sloped ridge, zero terrain, maximum depth, a saddle and mixed whitespace. Invalid input
cases include EOF, too few/many heights, a letter, a negative sign and UTF-8
non-ASCII bytes. `intended_invalid/` is a separate complete hosted entrypoint and
build selecting an unguarded runtime array read; it must be rejected for missing
index bounds, independently of runtime input rejection.

## Commands

From the repository root, using an exact verified compiler binary `OMEGA`:

```sh
"$OMEGA" update --project samples/cli/independent/watershed_spill_analysis --target linux_x86_64
# Read the generated build/package-manager/review-linux_x86_64.txt, accept the required program entry binding,
# then resume the reviewed proposal. See repository package-review documentation.
"$OMEGA" update --resume --project samples/cli/independent/watershed_spill_analysis
"$OMEGA" --check samples/cli/independent/watershed_spill_analysis/main.omg
"$OMEGA" run --both --keep samples/cli/independent/watershed_spill_analysis/main.omg < samples/cli/independent/watershed_spill_analysis/fixtures/bowl.txt
python3 samples/cli/independent/watershed_spill_analysis/reference/oracle.py
python3 samples/cli/independent/watershed_spill_analysis/reference/validate.py --omega "$OMEGA"
```

`build/validation/` holds captured commands, statuses, stdout/stderr and hashes.
`--both` verifies native/Terminal termination agreement; the CLI currently does
not compare streams. The validation runner separately checks emitted native
stdout byte-for-byte against the frozen oracle and requires actual Terminal
agreement rather than a declined comparison. It retains compiler-produced
artifacts using `--keep`. Runs are serial; do not execute concurrent Omega
compiles or Rust builds on the executor.

This model omits diagonals, fractional heights, missing cells, variable dimensions,
cell-area scaling, rainfall, infiltration, erosion, dynamic reservoir routing and
physical outlet capacities. These deterministic synthetic fixtures support
algorithm research, not hydrological forecasts or safety decisions.

For independent byte-exact Terminal output checks, the private reference harness
links the public complete-project and Terminal runtime APIs from one verified
Rust compiler build. It checks all fixtures through the exact produced Terminal
artifact with retained provisions, publishes that same native artifact, and runs
all inputs against the resulting executable. Declines, fuel exhaustion, crashes,
wrong exits, output differences and nonempty guest stderr fail validation. It
does not reconstruct artifacts or interpret checked trees.

```sh
mbx build -p omega --release --message-format=json > samples/cli/independent/watershed_spill_analysis/build/validation/rust-build.jsonl
python3 samples/cli/independent/watershed_spill_analysis/reference/build_validator.py --target-dir target/release --build-record samples/cli/independent/watershed_spill_analysis/build/validation/rust-build.jsonl
samples/cli/independent/watershed_spill_analysis/build/validation/terminal_validate samples/cli/independent/watershed_spill_analysis
```

The harness provisions the documented compiler stack size, selects Linux x86-64,
uses finite fixture files and isolated domain-exit delegation, and serializes all
executions. `build/validation/bulk/` retains both engines' exact stdout, stderr
and exit observations plus the normal published native executable.

The complete intended-invalid control uses `intended_invalid/invalid.omg`, so ordinary sample discovery does not treat its expected bounds rejection as a runnable application. Validate it separately with `omega --check` and require the index-within-length-25 diagnostic.

Current-main integration uses the allocator recovery already landed on main; the earlier private repair is retained only in the archived review candidate. The division/call/remainder regression remains in the ordinary corpus.

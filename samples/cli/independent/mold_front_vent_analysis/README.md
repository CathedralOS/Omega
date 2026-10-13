# Mold front and vent analysis

This is a declared synthetic cellular sample, not a constitutive model of
industrial injection molding. It models resin occupation and air connectivity
on a closed 4×4 cavity. All cells have one volume unit. A tick is one front
advance; no viscosity, pressure, temperature, compressibility, flow rate or
physical time is inferred. Four-neighbour connectivity, impermeable walls,
instant vent evacuation and permanently trapped air are explicit assumptions.

The entire implementation is private to this namespace. It depends only on the
existing standard console/entry providers. It imports no numerics library,
other sample application, shared algorithm export or worker candidate.

## Runtime input

One line, `WWWW GGGG VVVV BB`, uses four hexadecimal digits each for walls,
gates and vents, then two decimal digits for the fill budget. Bit `4*r+c`
selects row `r`, column `c`, with the least significant bit at top left.
Upper/lowercase hex, LF, CRLF and EOF after the exact seventeen characters are
accepted. A 32-byte bounded reader rejects full buffers. No gates, intersecting
wall/gate/vent roles, budgets above sixteen, and budgets below the gate count
are invalid. Vents may be empty: this is a blocked-vent experiment. The reader
consumes one line; later lines are outside this single-case invocation.

Example: `0000 0009 F000 05` injects at both top corners, vents through the
bottom row, and supplies five cell volumes. It fills indices 0 and 3 at tick
zero, then indices 1, 2 and 4 at tick one, leaving eleven vent-connected cells.
Budget tie breaking is explicitly row-major, so index 7 waits when the budget
runs out. Invalid input prints exactly `INVALID_INPUT\n` and exits 64.

## Model and output

Gate cells fill together at tick zero and consume budget. Before any advance,
air connected to an unfilled vent is classified as vented. Other air becomes
permanently trapped. Candidates are vented cells neighbouring any resin cell.
Each round takes one snapshot of those candidates, admits as many as remaining
budget allows in ascending cell order, stamps their arrival tick, and applies
them together. This deduplicates merging fronts without counting a cell twice.
After each batch, filling a vent can disconnect and trap the remaining air.
The process ends with an empty candidate set or an exhausted budget.

`model/fill.omg` implements a private sixteen-bit four-neighbour closure and
synchronous selection over a sixteen-entry arrival map. `input/parse.omg`
validates the initialized input prefix. `report/output.omg` emits:

- Arrival map: exact integer ticks, `#` walls and `.` unfilled cells.
- Final phases: resin `R`, trapped `T`, vent-connected air `A`, walls `#`.
- Every trapped connected region as a decimal bitmask and cell count, ordered
  by its lowest-index cell.
- Wall/resin/trapped/vented volumes, supplied budget, unused budget and rounds.

Every valid run checks `wall + resin + trapped + vented = 16`, complete
partition coverage, `resin = injected`, and `injected <= budget` before reporting
`OK` and exiting 0. Failure exits 70 with `INTERNAL_BALANCE_ERROR`.

## Independent expected artifacts

`validation/reference.py` uses sets and an explicit adjacency graph, rather
than the application's shifted bitmasks. Expected maps and region/volume
reports in `data/*.expected` were frozen before any Omega check or execution.
`validation/frozen.json` records the selected fixtures, batches, exhaustive
counts and tolerance. `validation/oracle.sha256` pins the reference and freeze.
The tolerance is **zero**: every tick, mask, count and byte must match exactly.
The reference parser is only a fixture reader; it is not an oracle for every
possible input spelling. Runtime grammar rejection is checked separately against
the declared input contract and the twelve authored invalid inputs.

The independent flood check enumerates all simple paths for every air/vent
assignment on an embedded 2×2 (256 combinations). Native validation enumerates
all wall/gate/vent/ordinary assignments with at least one gate and budgets from
the seed count through four (619 configurations), comparing complete stdout,
empty stderr and exit 0. Ten selected fixtures add larger merging fronts,
filled/blocked vents, an enclosed air pocket, multiple trapped regions, an
exhausted partial batch, a budget equal to seed volume, and input spelling
variants. Twelve invalid runtime inputs require exact stdout and exit 64.

These are substantive extensions beyond `algorithms/maze_flood` (one static
BFS distance map), `collections/heat_grid` (coordinate accumulation),
`simulation/grid_walk` (one declared path), and `simulation/game_of_life`
(neighbour population rules). Those sources were compared before authoring.
This sample's two coupled phases, closure after vent consumption, permanent
trapping, synchronous budget cut and cell-volume balance are different semantics.

The private decimal writer uses bounded subtraction: each digit takes at most
nine reductions. Its two-digit fields are at most 16, and its five-digit region
masks are at most 65535. The fixed place-value sequence preserves leading zeroes.

## Commands

From the repository root, with a compiler built from this checkout and its
matching library selected:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
omega update --project samples/cli/independent/mold_front_vent_analysis --target linux_x86_64
# Inspect the generated review and explicitly accept only its reviewed decisions,
# then use omega update --resume --project <same-directory> when required.
omega --check --target linux_x86_64 samples/cli/independent/mold_front_vent_analysis/main.omg
omega --target linux_x86_64 samples/cli/independent/mold_front_vent_analysis/main.omg
omega run --both --keep samples/cli/independent/mold_front_vent_analysis/main.omg < samples/cli/independent/mold_front_vent_analysis/data/merging.in
python3 samples/cli/independent/mold_front_vent_analysis/validation/validate.py /absolute/path/to/published/mold_front_vent_analysis
python3 samples/cli/independent/mold_front_vent_analysis/validation/validate.py --omega /absolute/path/to/omega
omega --check --target linux_x86_64 samples/cli/independent/mold_front_vent_analysis/controls/out_of_bounds/main.omg
```

Use the corresponding built executable on Windows and `python` if that is the
Python 3 command. Build metadata binds Linux x86-64/ARM64, macOS x86-64/ARM64 and
Windows x86-64, while execution evidence covers only the measured host. A single
`omega run --both` compares process termination only. `validate.py --omega`
instead compiles once with `omega run --both --inputs` and compares every
case's native and Terminal stdout, stderr and exit against the oracle.

The separate intended-invalid control must reject for `cannot prove index` at
`arrival[16]`, with no unrelated errors. Its bounds rule is documented in
`wiki/spec/language/numeric_values.md#collection-bounds`. Package-review, entry or ownership rejection is not a pass.
It is deliberately a complete compile-only source with its own build metadata,
so it can validate bounds independently of the application's console dependency.

`validation/EVIDENCE.md` records actual commands, compiler/binary/base identity,
resources, results and any denied or unsupported routes. Local generated output
stays under ignored `build/`. The clean candidate and separate review bundle
preserve source, frozen oracles and execution evidence without checking generated
compiler artifacts into the sample.

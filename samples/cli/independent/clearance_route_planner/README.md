# Clearance route planner (ADD11)

This Omega application plans a swept-disk path through a fixed six-node
two-dimensional roadmap. Two synthetic scenes contain rectangular walls and
one or two circular obstacles. The input selects radius 0..5 and scene 0..1.
Feasibility uses exact squared segment-to-center distance; tangency is allowed.
Each edge costs `floor(1000 * Euclidean length)`. The planner minimizes that
integer cost over the finite roadmap and emits coordinates and clearance.

The program independently rechecks a selected path. If no path exists, it
checks a separating cut containing the start and excluding the goal. Blocked
endpoints produce a separate diagnostic. This establishes results for the
declared roadmap; it does not search every curve in the plane.

## Input and frozen results

Input is exactly `radius scene` followed by LF, for example `2 0\n`. A route
exits 0; an infeasible roadmap or blocked endpoint exits 2. Malformed input,
including EOF without LF, exits 64. Program stderr is empty.

All twelve radius/scene combinations and ten malformed inputs are frozen in
`data/expected.json`. Its original SHA-256 is
`f595b43edd2a90a959b0c90a23e84845711224faf0d3e9d551836970947d5897`.
Input bytes, output bytes, exits, coordinates, costs and clearances are retained.
Feasibility and quantized path costs have zero tolerance. Clearance truncation
satisfies `0 <= exact clearance - reported/1000 < 0.001`.

The independent reference uses projection coordinates and exact Python
fractions, enumerating all 65 simple start-to-goal paths. Omega uses a cross
product distance formula and a six-node Dijkstra search. The reference checks
the frozen file and refuses to overwrite it unless explicitly freezing a new,
absent file. Validation never changes expected data or package decisions.

## Recovered provenance

The original source candidate was
`cbecd3b1c30c9961dc195aa8a1104c7add668660`. These application, invalid-control and
borrow-witness bodies were reconstructed from its retained authoring literals
and later recorded text substitutions. Their derived hashes are recorded in
`validation/recovery.json`. The complete original archive's final per-file
manifest is unavailable, so full archive byte equivalence is not claimed.
The original reference and scene literals reproduce the independently recorded
frozen oracle hash exactly.

Historical review, lock, approval commands and holds supplied provenance only.
Current admission and execution use the ordinary project workflow. The separate
`bounded_hoist_planner` project preserves the later synthetic height-profile
reconstruction and its distinct frozen suite.

## Normal qualification

Use the compiler built from the recorded source revision and set
`OMEGA_LIBRARY_ROOT` to this checkout's `source/library`. From repository root:

```sh
sample=samples/cli/independent/clearance_route_planner
"$omega_binary" update --project "$sample" --target linux_x86_64 --offline
# Inspect the current proposed dependency/source decisions and findings.
# Resume the same proposal after its exact decisions are reviewed.
"$omega_binary" update --resume --project "$sample"
"$omega_binary" --check --target linux_x86_64 "$sample/main.omg"
python3 "$sample/reference.py"
python3 "$sample/validation/build_runtime.py" --omega "$omega_binary" --output "$sample/build/runtime-validator"
python3 "$sample/validate.py" --omega "$omega_binary" --runtime-harness "$sample/build/runtime-validator"
```

The private observer compiles through the normal prepared native report, runs
the actual native executable and the retained public Terminal artifact with
that report's runtime provisions, and checks all 22 stdout/stderr/exits.
CLI `--both` is an additional check of both engines' termination reports.

`invalid/main.omg` is the original complete control: an unguarded ordinal
indexes six nodes and must reject with ``cannot prove index `ordinal` ``.
An unrelated package, parser or provider refusal does not satisfy that oracle.
`repro/main.omg` retains the original bounded-byte loan witness, whose authored
exit is 70. Linux execution does not imply matching-host native qualification
of the declared macOS ARM64 or Windows x86-64 entries.

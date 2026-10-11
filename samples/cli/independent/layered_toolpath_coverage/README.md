# Layered toolpath coverage

A complete bounded Omega CLI for an explicitly synthetic fabrication model.
The supplied region is a union of integer unit cells in up to four layers.
The scientific sample treats unit cells as deposition strips and travel as
Manhattan waypoints. Extrusion physics, collision clearance, acceleration, thermal
and bead-spread effects are outside this model.
Unit-width deposition covers exactly the cell strip assigned to its lane.

Input consists of ASCII digit tokens (whitespace may separate them). The first
five tokens are width (1–8), height (1–8), layer count (1–4), orientation
(0 horizontal, 1 vertical), and lane stride (1 or 2). Then supply exactly
width × height × layers occupancy tokens, each 0 or 1, in layer/row/column order.
Only whitespace may follow. EOF, malformed headers, bad cells and extra tokens
produce `ERROR input` and exit 64 before any segment is emitted. Input waits
for finite stdin ending at EOF; the storage bound is 256 cells.

For each layer, the planner scans ascending lanes starting at zero. It deposits
maximal contiguous occupied runs in the positive axis direction. A gap ends a
run; the next run requires a travel waypoint record. Each record has a kind
(`D` deposition, `T` travel) and six four-digit integer coordinates:
`start_x start_y start_z end_x end_y end_z`. Deposition coordinates are cell
strip edges along the deposition axis, and lane/layer indices on the other
axes. Travel records mean an axiswise Manhattan waypoint move (z, then y,
then x) with deposition off. They do not assert safe physical clearance.
The first waypoint is (0,0,0); empty layers do not generate motion by themselves.

The final `AUDIT` reports required region cells, uniquely covered region cells,
missed region cells, deposition visits outside the region (`waste`), repeated
visits (`duplicate`), deposited unit length, Manhattan travel length, and D/T
record counts. Coverage is independently enumerated from the mask and the
private visit array after path generation. Stride two intentionally leaves
missed lanes, demonstrating a nonzero coverage deficit without depositing into
gaps. Waste and duplicate counts should be zero for this planner.

The application owns all storage, parsing, algorithms and rendering under
`domain/`. Its only dependency is the stable bundled std Console provider.
No in-progress numerics library or other worker output is imported.

## Frozen acceptance

`ORACLE.json` and `data/*.out` were frozen before the first compiler check.
All geometry, lengths, counts and output bytes use exact integer equality;
numerical tolerance is **zero**. The oracle manifest SHA-256 is
`d4f566be2f960fa708f16d9cd039950c65c4c890969d258e3245ddd8d1950a6e`.
`reference.py` constructs independent rectangle sets, subtracts holes, unions
islands, groups mathematical intervals, and enumerates emitted D records again
when validating. It is an oracle and validator; execution is the Omega program.
Do not re-freeze expectations to make a failed execution pass.

Nine fixtures exercise rectangles, holes, disconnected islands, three differing
layers, vertical orientation, stride-two undercoverage, empty regions, a unit
cell, and all 256 cells. Ten runtime-invalid fixtures test dimension, orientation,
stride, layer count, malformed cells, truncation, trailing nondigits and extra digit tokens, nondigits and EOF.
The complete separate control under `controls/out_of_bounds/` intentionally
writes index 256 into a 256-cell array. It must reject with `cannot prove index`;
package, parser, provider or unrelated proof rejection is not a pass.

Existing samples compared before coding: `algorithms/maze_flood` computes BFS
distances, `collections/heat_grid` accumulates a fixed grid, and
`collections/run_length_encode` groups bytes. None emits layered fabrication
segments, distinguishes non-depositing gap travel, or audits cell coverage,
undercoverage and wasted deposition. This workflow composes those primitive
families for a different domain and supports supplied runtime regions.

## Commands

From the repository root, with `OMEGA_LIBRARY_ROOT` set to this checkout's
`source/library`, and a compiler built from the recorded base:

```sh
omega update --project samples/cli/independent/layered_toolpath_coverage --target linux_x86_64
# Inspect the generated review, accept only the reviewed decisions, then:
omega update --project samples/cli/independent/layered_toolpath_coverage --resume
omega --check samples/cli/independent/layered_toolpath_coverage/main.omg
omega run samples/cli/independent/layered_toolpath_coverage/main.omg < samples/cli/independent/layered_toolpath_coverage/data/hole.in
python3 samples/cli/independent/layered_toolpath_coverage/reference.py --cli /absolute/path/to/omega
python3 samples/cli/independent/layered_toolpath_coverage/reference.py --cli /absolute/path/to/omega --both
omega --check samples/cli/independent/layered_toolpath_coverage/controls/out_of_bounds/main.omg
```

The final command is expected to exit 1 with the intended bounds diagnostic.
Native and `--both` results/logs are retained in ignored `build/`. The portable
Console supports Terminal comparison on this Linux host; compare stdout against
the frozen oracle separately because CLI comparison itself compares termination.
Review and custody denials must be reported without bypass or acceptance retries.

See `VALIDATION.md` for actual routes, identities and any remaining blockers.
Linux x86_64 validation passed all 19 native inputs with exact output checks,
Terminal termination comparisons, and the separate intended bounds rejection.
The witnessed native allocator gap required a bounded, independently reviewed
fixed-use operand transport repair; its complete runtime regression and focused
compiler controls are described in that record.
After independent review and integrated validation, publication uses `tools/landing.py`
and the shared main-only queue. There are no remote worker branches.

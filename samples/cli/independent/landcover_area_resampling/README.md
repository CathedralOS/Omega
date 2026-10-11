# Land-cover area resampling

This complete Omega CLI resamples a bounded 2×2 categorical raster into a
selected destination grid. It computes every source/destination cell intersection,
reports forest (1), water (2), urban (3), source no-data (0), and uncovered area as
exact fractions of the **full destination cell**. No-data never wins dominance;
equal positive class areas choose the lowest identifier. A cell with no valid
coverage has dominant class 0. Outside coverage is separate from no-data.

The explicitly declared scientific model is a planar equal-area coordinate system:
integer metre ticks, half-open axis-aligned rectangles, constant category inside
each source cell. There is no reprojection, latitude/longitude interpretation,
interpolation, probability model or invented precision. Fractions are unreduced
integer numerator/denominator pairs; absolute and relative numerical tolerances
are zero. Wrapping storage arithmetic is explicit; input admission bounds ensure
all actual intermediates are nonnegative and at most 256. Three-digit output is
therefore sufficient and never rounds. The source footprint is [0,4)×[0,4), with
2×2 metre cells in row-major order [forest, water, urban, no-data].

Enter one fixture digit followed by LF. The four-byte input buffer rejects EOF,
unterminated lines, extra bytes before LF and full input buffers. It processes
one line; subsequent lines remain unread. Modes 7–9 deliberately supply invalid grid
width, category and grid extent and must exit 64 before resampling. Modes 1–6 are:

| Mode | Destination | Distinction |
|---|---|---|
| 1 | aligned 2×2 cells, 2×2 grid | exact identity including no-data |
| 2 | origin (1,1), 2×2 cells, 2×2 grid | shifted, partial coverage, three-class tie |
| 3 | one 4×4 cell | coarsening, three-class tie, no-data fraction |
| 4 | origin (6,0), 2×2 cells, 2×2 grid | fully disjoint, no dominant class |
| 5 | origin (0,0), 3×3 cells, 2×2 grid | unequal resolution, ninth fractions |
| 6 | aligned, all source cells no-data | covered no-data versus uncovered area |

Every output cell closes to its complete area. The application also sums each
class across destination cells and compares it with source cells clipped once
against the complete destination footprint. It audits no-data and full-area
closure as well. The independent reference in `data/reference.py` enumerates
unit squares, using no rectangle intersection calculation. `data/fixtures.json`
and six complete stdout files were frozen before compiler execution; validation
never regenerates them.

Existing sample comparison: `collections/heat_grid` updates values in a grid,
`simulation/grid_walk` walks a coordinate path, `collections/matrix_multiply`
performs algebra, `rendering/histogram` renders counts and `proofs/shapes_area`
computes individual shape areas. None performs categorical cell intersection,
full-cell fractions, missing coverage, deterministic dominance and class
conservation. This workflow has its own private algorithms and no numerics imports.

From the repository root, using a compiler built at the recorded base:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
omega --check --target linux_x86_64 samples/cli/independent/landcover_area_resampling/main.omg
omega update --project samples/cli/independent/landcover_area_resampling --target linux_x86_64
# Read the pending review. Accept only its reviewed decisions, then:
omega update --resume --project samples/cli/independent/landcover_area_resampling
omega --target linux_x86_64 samples/cli/independent/landcover_area_resampling/main.omg
python3 samples/cli/independent/landcover_area_resampling/validate.py /absolute/path/to/native/executable --output samples/cli/independent/landcover_area_resampling/build/validation/native
# Build the documented corpus_runner test target once, then reuse its executable.
python3 samples/cli/independent/landcover_area_resampling/validate_terminal.py /absolute/path/to/corpus_runner --output samples/cli/independent/landcover_area_resampling/build/validation/terminal
printf '2\n' | omega run --both samples/cli/independent/landcover_area_resampling/main.omg
omega --check samples/cli/independent/landcover_area_resampling/controls/out_of_bounds/main.omg
```

The last command must reject specifically for the unprovable array index; an
unrelated parse, package or entry diagnostic is not acceptance. The control is a
complete independent source program, not a commented line in the valid sample.
Native execution is declared for Linux x86-64; other targets have no advertised
execution evidence. Generated products and validation observations belong under
`build/`, while reviewed lock state follows the ordinary package workflow.

`run --both` on the host compares process exit status. An explicit `run --target`
produces an image without execution. The Terminal validator uses the documented
corpus route to compare complete stdout, stderr and exit oracles for both engines.
It stages byte-identical application sources in fifteen private scratch fixtures,
adapts only the relative std dependency path, and removes the fixtures afterward.
The engine consumes the native report's retained Terminal artifact; there is no
checked-tree fallback or golden rewrite.

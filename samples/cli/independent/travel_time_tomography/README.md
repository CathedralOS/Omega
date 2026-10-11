# Travel time tomography (NEW087)

A complete synthetic two-cell inverse application: read three ray length/time
observations, minimize the sum of squared time residuals subject to per-cell
slowness bounds, and print the recovered field, signed residuals, objective,
active bounds, matrix rank, nullity, and a nullspace witness. This explicitly
assumes straight rays, uniform slowness within each cell, and equal weights.
Inputs are synthetic; this is not a validated geophysical inference tool.

The entire application lives here. `domain/model.omg` owns rays and results;
`domain/solver.omg` owns its private Gram matrix, stationary-point calculation,
and exhaustive box-edge minimization; `io/input.omg` owns bounded parsing;
`io/report.omg` owns numeric output. It depends only on the repository's stable
core and standard console providers. No in-progress numerics package or worker
candidate is imported.

Existing samples were checked for semantic overlap before authoring. `dot_product`
reduces a fixed vector, `matrix_multiply` computes a forward matrix product,
`stats_compute` accumulates fixed statistics, and `vector_distance` computes a
magnitude. None accepts an observation system and recovers a box-constrained
field, distinguishes inconsistent observations from rank deficiency, or supplies
nullspace witnesses. Searching samples for `least.squares`, `tomograph`,
`slowness`, and `inverse problem` found no matching workflow at the recorded base.

Input is exactly 13 unsigned decimal integers, each 0..99, separated by ASCII
space, tab, LF or CR, terminated by EOF:

```text
lower_cell1 upper_cell1 lower_cell2 upper_cell2
ray0_length1 ray0_length2 ray0_time
ray1_length1 ray1_length2 ray1_time
ray2_length1 ray2_length2 ray2_time
```

Bounds must be ordered. There are at most 255 input bytes, including whitespace.
Signs, fractional tokens, three-digit tokens, extra/missing tokens, and overlong
streams are rejected with exit 64. An inconsistent or unidentifiable valid
system still produces a best fit and exits 0. An empty observation column is
valid and reported as unidentifiable. Lengths and times use arbitrary consistent
synthetic units; slowness is time per length. The input restriction is deliberate
and published, rather than a parser pretending to support arbitrary floats.

Numeric output uses signed integer micro-units: divide values by 1,000,000.
Conversion truncates toward zero. Rank and bound labels are categorical. The
reported rank is that of the observation matrix, independent of constraints:
fixed bounds may select one field even when the observations cannot identify it.
Rank-one solutions prefer minimum Euclidean norm among candidates with
exactly equal computed binary64 SSE; this tie-break is not a uniqueness claim. Rank-zero selects the lower corner and reports both basis vectors.
`fit=consistent_within_1e-12_sse` means SSE < 1e-12, including constrained fits.

Integer input makes Gram products exact in binary64. The determinant is an
integer, so `determinant > 0.5` distinguishes rank two from rank one exactly in
this bounded model. The trace test similarly distinguishes rank zero. Analytical
edge minimization includes corners, including degenerate boxes and unobserved
columns. The solver does not silently add regularization. `near_parallel.txt`
has determinant one and challenges the rank cutoff; `large.txt` challenges
output width and upper bounds. All input arithmetic stays in bounded Exact
integer semantics; inverse arithmetic uses ordinary binary64 semantics.

The oracle was frozen before compiler check or execution in
`validation/oracle.json` (SHA-256
`79514057110d7d0d8ef6a43203d0390814ff73a4b6fa4a7675b40aecce512358`).
Expected fields are independently declared rational values, certified by exact
convex KKT conditions; residuals and SSE follow exact rational arithmetic. Rank
is checked using cross products of ray pairs, independently of the Omega Gram
calculation. Field, residual, and SSE tolerances are absolute 2e-6 (including
micro-unit output truncation); rank, nullspace residuals, active labels, and
exit codes are checked exactly. The verification script never rewrites oracles.

From the repository root, with a matching compiler/library:

```sh
python3 samples/cli/independent/travel_time_tomography/validation/verify.py --oracle-only
omega --check samples/cli/independent/travel_time_tomography/main.omg
omega update --project samples/cli/independent/travel_time_tomography --target linux_x86_64
# Inspect the generated review and accept only decisions authorized for this
# local std dependency, then use the documented update --resume workflow.
omega run samples/cli/independent/travel_time_tomography/main.omg < samples/cli/independent/travel_time_tomography/data/exact.txt
python3 samples/cli/independent/travel_time_tomography/validation/verify.py --compiler /absolute/path/to/omega --both
# Run an already published executable through all fixtures without recompilation:
python3 samples/cli/independent/travel_time_tomography/validation/verify.py --native /absolute/path/to/published/executable
```

For complete Terminal output validation, build the private test consumer using
`mbx` (Cargo if unavailable), then run it and the same frozen verifier:

```sh
mbx build -p new087-terminal-validation
new087-terminal-validation samples/cli/independent/travel_time_tomography samples/cli/independent/travel_time_tomography/build/terminal
python3 samples/cli/independent/travel_time_tomography/validation/verify.py --captured samples/cli/independent/travel_time_tomography/build/terminal
# Use the executable path saved in build/terminal/native-path.txt for --native.
```

This test-only Rust consumer uses the ordinary public package preparation and
native facade, keeps the produced Terminal artifact and exact provisions, and
executes each input in a new isolated Terminal instance with finite stdin.
It captures both streams and exact completion codes before publishing the same
native product. It never accepts packages, replays source, supplies expected
result callbacks, or substitutes a checked-tree interpreter. The application
and all tomography algorithms remain Omega.

Only Linux x86_64 is selected in the current build metadata. `--both` compares
native and Terminal termination on the same produced artifact. The CLI does
not compare their stdout; the verifier separately checks captured native output.
An unsupported Terminal comparison must be reported as DECLINED, never agreement.

The separate complete intended-invalid project is
`controls/invalid_bounds/invalid.omg`. It reads `cells[2]` from a two-element ray
length array. Its only intended diagnostic is `cannot prove index`; parse,
package, provider, or entry failures do not pass this control. Check it separately:

```sh
omega --check samples/cli/independent/travel_time_tomography/controls/invalid_bounds/invalid.omg
```

Generated programs, captured stdout/stderr, and per-case JSON results belong
in ignored `build/`; frozen oracles and input fixtures are committed. Validation
observations, exact compiler identity, transfer identity and current acceptance
status accompany the candidate, rather than treating authored code as proof of
execution.

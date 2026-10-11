# Bounded micrograph regions

A complete Omega command-line workflow for synthetic grayscale micrographs.
It reads an image, applies an inclusive threshold, floods foreground regions,
and emits a label image plus exact region measurements. It uses stable bundled
Console providers and private application algorithms; no external numeric package
or active worker implementation is required. These are synthetic pixels, with no
medical interpretation.

The nearest existing sample is `algorithms/maze_flood`, which computes a single
source shortest-distance map over a fixed maze. This workflow instead parses
arbitrary bounded runtime images, labels all connected components, measures each
region, supports distinct connectivity rules, and rejects malformed input.
`collections/heat_grid` demonstrates coordinate indexing but does not segment or
measure regions. `rendering/mandelbrot` generates pixels rather than analyzing
input. None provides this workflow.

Input is ASCII unsigned decimal, separated by spaces, tabs, CR or LF:

```text
width height threshold connectivity
pixel pixel ...
```

Width and height are 1 through 8, threshold and pixels are 0 through 255, and
connectivity is 4 or 8. Supply exactly width × height pixels, then EOF. Tokens
have at most three digits. Empty input, dimensions outside the bounds, unknown
connectivity, missing or excess pixels, negative values and malformed tokens
produce the documented error line and exit 2. Console I/O failures remain the
provider's Trap behavior. Valid input exits 0; an internal capacity violation
would exit 3.

Foreground means intensity >= threshold. A threshold tie is included, including
zero at threshold zero. Seeds are visited in row-major order, so component IDs
are deterministic even when connectivity changes. Zero denotes background.
A FIFO flood marks a pixel before enqueueing; each pixel enters the 64-slot
private queue at most once. Coordinates are checked before row-major conversion,
so opposite row edges cannot become neighbors. There are no heap allocations.

Output begins with an ASCII PGM (`P2`) label image, max value 64, followed by a
`regions N` report and column header. The validation tool splits actual output
into `.pgm` and `.regions.txt` artifacts. Coordinates start at zero at the upper
left, x points right, y down. Bounds are inclusive. Centroids use pixel centers
at integer coordinates and are exact unreduced fractions `sum_x/area` and
`sum_y/area`. There is no floating-point rounding or tolerance; all output bytes
are checked exactly.

`main.omg` owns the entrypoint; `domain/input.omg` owns bounded parsing and image
storage; `domain/regions.omg` owns labeling and measurement; `domain/output.omg`
owns serialization. `build.omg` binds the exact entry machine for Linux x86-64,
Linux ARM64, Windows x86-64 and macOS ARM64 using existing std providers. Only the
available Linux x86-64 executor is validated here.

From the repository root, with an exact compiler binary chosen as `OMEGA`:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
SAMPLE=samples/cli/independent/micrograph_regions
"$OMEGA" update --project "$SAMPLE"
# If a review is required, inspect the proposal and accept its local std/entry
# decisions, then resume the normal documented review flow:
"$OMEGA" update --resume --project "$SAMPLE"
"$OMEGA" --check "$SAMPLE/main.omg"
"$OMEGA" run "$SAMPLE/main.omg" < "$SAMPLE/data/synthetic4.txt"
"$OMEGA" run --both "$SAMPLE/main.omg" < "$SAMPLE/data/synthetic8.txt"
# To compile once and reuse the emitted executable for all native inputs:
"$OMEGA" --build-dir "$SAMPLE/build/native" "$SAMPLE/main.omg"
# Set NATIVE_EXE to the exact emitted executable named in that report.
python3 "$SAMPLE/validate.py" --omega "$OMEGA" --mode native --native-executable "$NATIVE_EXE"
python3 "$SAMPLE/validate.py" --omega "$OMEGA" --mode both
```

Run one compiler execution at a time. `--both` uses finite redirected stdin,
compares native termination with the produced Terminal artifact, and must report
`interp exit: <expected>`; a declined comparison does not pass validation.
The checker and native/Terminal validation are separate requirements. The CLI
comparison does not compare output streams. The private validation harness uses
`compiler::runtime::execute` with the normal prepared native report's exact
Terminal artifact and retained provisions, and checks buffered Terminal stdout,
stderr and exact exit for every frozen case. It does not recheck trees or rerun
Build per case. Each execution receives its own isolated runtime instance.

Build this validation tool after the compiler build has completed. For a verified
mbx release build, reuse its exact recorded Rust dependency closure:

```sh
python3 "$SAMPLE/validation/build_terminal.py" --omega "$OMEGA" --output "$SAMPLE/build/ind23-terminal-validation"
TERMINAL_HARNESS="$SAMPLE/build/ind23-terminal-validation"
python3 "$SAMPLE/validate.py" --omega "$OMEGA" --mode terminal --terminal-harness "$TERMINAL_HARNESS"
```

The link helper checks the CLI output hash against its build-unit receipt and
selects exact libraries using the recorded dependency fingerprints. It supplies
both full `.rmeta` metadata and corresponding `.rlib` code, as the pinned toolchain
requires. Its build receipt records the command and every library SHA-256.
A matching mbx CLI build receipt is required: build the compiler with
`mbx build -p omega --release --locked -j 2` before linking the validator.
The validator is a directly linked private tool, with no separate Cargo project.
Build only one Rust project at a time. The harness is validation only; the
standalone Omega application has no Rust helper at runtime.

`expected/frozen.json` freezes input and stdout SHA-256 identities before Omega
execution. `reference.py` constructs an independent union-find oracle, differing
from the application's queue traversal. It is an authoring/reference tool,
not the delivered application and never invoked by `validate.py`. Do not
regenerate expected files to resolve an application mismatch.

The suite has 19 valid cases: exact synthetic grayscale images under both
connectivities, touching lobes, an empty foreground, threshold ties at 128 and
255, threshold zero, a singleton, a full 8 × 8 component, 4/8-connected
checkerboards, opposite row edges, a width-one image, and six deterministic
random variants. Twelve invalid runtime cases exercise bounds, truncation,
trailing input, lexical failures and the token limit. Reports compare every
label, area, centroid numerator/denominator and bounding-box coordinate.
Artifacts and receipts go under ignored `build/validation/{native,both,terminal}/`.

The repository generic native sample gate supplies EOF and checks the documented
empty-input rejection (exit 2). The private validator above supplies the full
valid-image oracles; the generic gate does not replace them.

The separate complete intended-invalid control deliberately stores past the
64-label capacity. It has its own build and entry binding; it is not used by the
application and is named `control.omg` so ordinary sample discovery does not
mistake it for a runnable positive sample.

```sh
"$OMEGA" update --project "$SAMPLE/intended_invalid"
"$OMEGA" --check "$SAMPLE/intended_invalid/control.omg"
```

The control must reject specifically on `labels[64]`'s unsatisfied bound,
not on a package, provider or unrelated source error. Record its diagnostics
and nonzero status separately from runtime input rejection.

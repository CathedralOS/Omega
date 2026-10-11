# Bounded spectrometer attribution (IND19)

This complete Omega application attributes regions in a synthetic spectrum to
an input reference-line catalog. It has no active-worker dependencies, private
package dependencies, external instruments, or services. All acquisition,
region integration, matching, and output code belongs to this directory.
`spectrometer/` separates the model, parser, integration, matching, and reporting
responsibilities; they are explicitly imported modules under the application’s `spectrometer` namespace.

Existing samples were compared before authoring: `sensor_min_max` computes
scalar extrema, `longest_run` counts repeated bytes, and the audio samples
synthesize signals. None subtracts a baseline, integrates spectral centroids,
or attributes observed regions to a supplied catalog. This sample exercises
that combined workflow with runtime data rather than renamed constants.

## Input

Supply exactly 27 whitespace-separated signed decimal integers, then EOF:

1. Sixteen intensity bins, each 0..1000.
2. Baseline intensity at bin 0 and bin 15, each 0..1000.
3. Detection threshold, 1..1000 counts.
4. Axis origin, 0..900000 milli-nanometers.
5. Bin spacing, 1..5000 milli-nanometers.
6. Additive calibration correction, -5000..5000 milli-nanometers.
7. Inclusive catalog matching tolerance, 0..5000 milli-nanometers.
8. Four reference-line coordinates, 0..1000000 milli-nanometers.

Only ASCII space, tab, CR and LF are delimiters. Minus requires a following
digit; plus signs and decimal points are rejected. Parsed magnitudes are bounded
by one million before use. Missing or extra tokens, malformed values and values
outside their field's bounds print `invalid spectrum input` and exit 2.

The baseline is the linear interpolation between the supplied endpoints.
Internally residuals, area and moments retain integer fifteenths: residual
numerator = 15*sample - baseline_start*(15-index) - baseline_end*index.
Threshold comparison uses 15*threshold. With these accepted integer inputs,
all accumulations and scaled output numerators remain below 2^53 and are exact
in f64. Centroid divides the scaled moment by the scaled area, cancelling the
factor fifteen. Output scales its exact numerator before division so exact
half-thousandth ties remain representable half-integers for rounding.
Residuals are clipped at zero. A bin is retained if its residual is greater
than or equal to threshold. Consecutive retained bins form a single region.
There is no deconvolution: overlapping components connected above threshold
remain one observed region, and ambiguous catalog results remain explicit.

For region R, area = sum(residual[i]), centroid =
sum(residual[i] * (origin + spacing*i)) / area + calibration.
This is rectangular bin integration on a uniform axis; area is measured in
counts, not physical continuous energy. Sixteen bins can form at most eight
regions. The code also reports a defensive capacity failure as exit 3.

All catalog IDs are their one-based input order. Every line within tolerance
is listed. Zero candidates means missing, one means unique, and two or more
means ambiguous. `nearest` is the minimum-distance candidate; exact ties keep
the earlier catalog ID. It is a deterministic preference, not an ambiguity
resolution. Repeated reference coordinates keep distinct IDs.

## Output and independently frozen oracle

Each report contains a region count and a row per region: inclusive bin bounds,
`area_milli` (counts times 1000), `centroid_micro` (milli-nanometers times 1000),
candidate count, preferred catalog ID, classification and all candidate IDs.
Numbers use nine decimal digits with a leading minus when needed. Numerical
output is rounded to nearest, with exact half ties away from zero.

`reference.py` computes exact `Fraction` results by constructing a threshold
mask and reducing maximal runs. It neither executes Omega nor reads its output.
The input fixtures, analytic rows, output oracles and numerical tolerances were
frozen in `data/oracle.json` and committed at `c4e961170c` before successful Omega compilation or execution. Discrete results and ordinary
output are exact; area and centroid quantization have an absolute tolerance of
0.000501 counts and milli-nanometers, respectively. Matching uses unrounded
centroids. Nothing in the application imports the reference implementation.

Hand calculations for `isolated.input`: after subtracting baseline 10+i,
bins 2..4 have weights [2,6,2], area 10, moment 5030000, centroid 503000.
Bins 10..11 have weights [4,8], area 12, moment 6128000, centroid 1532000/3.
The first region matches ID 1. The second matches IDs 2 and 3, preferring ID 2.
For overlap, weights [3,7,8,7,3] at bins 2..6 integrate to area 28 and centroid
504000, leaving three plausible reference lines rather than splitting a peak.

Twelve valid fixtures cover isolated and overlapping peaks, inclusive threshold
and tolerance boundaries, noise-only input, missing catalog lines, uncorrected
and corrected calibration, duplicate/equidistant reference lines, eight regions
including the final bin, a fractional sloping baseline, a negative calibrated
coordinate, and baseline clipping. Seventeen runtime-invalid fixtures cover
syntax, count, magnitude, and domain validation failures.

## Build and run

From the repository root, with a compiler built from that tree and matching
library content (substitute the absolute binary path):

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
OMEGA=/absolute/path/to/omega
APP=samples/cli/independent/spectrometer_attribution
"$OMEGA" update --project "$APP" --target linux_x86_64
# Inspect the emitted review, accept the expected local std/entry decisions,
# then resume if the update requests review.
"$OMEGA" update --resume --project "$APP"
"$OMEGA" --check --target linux_x86_64 "$APP/main.omg"
"$OMEGA" run --both --keep "$APP/main.omg" < "$APP/data/isolated.input"
python3 "$APP/validate.py" --omega "$OMEGA"
```

The installed host providers are the ordinary std console. Build metadata binds
exact hosted program-entry slots. Linux x86-64 is the execution acceptance host;
other bindings are portability declarations, not unexecuted test claims.
`run --both` compares native and Terminal termination only. The optional corpus
validation route checks each engine's stdout and stderr independently against
the same frozen files; it never substitutes checked-tree execution for Terminal.

`intended_invalid/` is a separate complete executable project that must be
rejected for Exact i32 overflow (`2147483647 + 1`). It has its own build metadata
and entry binding. It is not imported by the valid application. The validation
script checks rejection independently and requires the overflow diagnostic.
To verify both execution engines' streams using the repository harness, build
its runner from the same compiler revision, then stage only temporary copies of
this sample (the script removes its own fixture group afterward):

```sh
CARGO_TARGET_DIR=/absolute/private/target CARGO_BUILD_JOBS=4 \
  mbx test -p compiler --test corpus_runner --release --no-run
python3 "$APP/corpus_review.py" --runner /absolute/path/to/corpus_runner
python3 "$APP/corpus_review.py" --runner /absolute/path/to/corpus_runner --source-control-only
```

This uses one corpus worker, a 600-second per-fixture cap, the ordinary reviewed
repository fixture package-input route, and the exact native report's Terminal
artifact. All 32 cases require native and Terminal stdout/stderr matches plus
matching authored exits. Generated artifacts and logs stay in the ignored
`build/` directory. The second command checks the separate complete overflow
project through the checked corpus route and requires its intended diagnostic.
For a bounded replay, append `--case isolated` (repeat `--case` for more cases).

The private runtime adapter avoids recompiling identical application sources for
each input. It uses the repository's reviewed fixture package-input factory,
without persisting production package acceptance. Its Rust code contains no
spectrum calculations: it requests the ordinary Check and Native products,
executes all inputs with the native report's exact Terminal artifact and
provisions, then publishes and executes that native image. Both engines must
match every frozen byte and exit independently. It retains actual streams,
exits, the native image, and canonical Terminal bytes in `build/runtime_matrix/`.

```sh
CARGO_TARGET_DIR=/absolute/private/target CARGO_BUILD_JOBS=4 \
  mbx build --manifest-path "$APP/Cargo.toml" --release --locked
python3 "$APP/runtime_matrix.py" --driver /absolute/private/target/release/ind19-spectrum-validation
```

The adapter is registered as a repository Cargo workspace member and keeps all
its implementation in this sample. It depends only on stable compiler facilities. It does not replace the Omega entrypoint, call the Python
reference, relax compiler checks, or select the checked-tree interpreter.

The optional `--terminal-only` diagnostic requests Check and the standalone
canonical Terminal product through the ordinary compiler facade. Its artifacts
go to `build/terminal_matrix/`. The original review candidate
`3194b541909601ea2fa987eab8e1dc796148eb69` produced that artifact, but
execution rejected four missing exact runtime-provider installations. It provides no execution acceptance. The default command uses
the Native report's retained Terminal artifact and provisions and requires
all 32 inputs to pass both engines.

The original 29-case acceptance and immutable review bundle are preserved.
Independent review exposed two additional accepted-input boundaries: a descending
baseline (75 to 0) at bin 11, intensity 22, threshold 2; and centroid 1001/2000,
whose output must round to 501 micro-nanometers. Three supplemental cases,
including the negative half-thousandth tie, were independently frozen in
`data/boundary_oracle.json` before repaired execution. The original `oracle.json`,
all original inputs/expected outputs, tolerances and `reference.py` are unchanged.
The validation commands now require all 32 cases in both engines. No production
package acceptance is granted, and other host bindings remain unexecuted.

The separate `controls/value_equality/` project checks ordinary value-returning
float equality on runtime input `A`, requiring both equal and unequal results.
It guards the published compiler repair for copied resultless algebra-law
predicates: detached copies retain their rooted occurrence meaning, while
executable equality retains ordinary declared float dispatch. Run it
with the same verified adapter:

```sh
/absolute/private/target/release/ind19-spectrum-validation "$APP/controls/value_equality" --fixture
```

This control requires Check, Native and the Native-retained Terminal product,
exit 0, exactly `float equality: equal and unequal` followed by a newline, and
empty stderr. Its expectations are source controlled before execution.

The optional read-only identity check inspects the sealed Check product and
requires the exact reachable `spectrometer_equality_control::same` entry body
to retain one authored `Equal` operator occurrence selecting the public
`Float::equal` declaration. It grants no compiler authority:

```sh
/absolute/private/target/release/ind19-spectrum-validation "$APP/controls/value_equality" --selection-identity
```

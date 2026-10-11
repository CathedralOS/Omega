# Bounded position and velocity estimation

Validation is pending package-policy approval. The separate invalid control
matched its exact rejection; application checking reaches the dependency-owned
entry-binding gate. Native and Terminal execution have not occurred: automatic
approval review rejected std policy acceptance (285 decisions, including process
and filesystem capabilities). See `ASSIGNMENT.md`; no acceptance lock exists.

IND06 is an independent, two-dimensional linear Gaussian state estimator.
All algebra, prediction, scalar observation updates, parsing and formatting
are private to this application. The only external package is the existing
standard console provider. It does not use the numerics library or a worker's
candidate. It reads runtime stdin and writes a complete trace to stdout.

The initial estimate is position 0, velocity 1, with identity covariance.
Each record advances time once with F = [[1, dt], [0, 1]] and Q = q I.
P observes position, V observes velocity, and M supplies no observation.
A scalar observation has variance r. The update uses K = P h / S,
S = h' P h + r, and Joseph covariance
(I - K h') P (I - K h')' + r K K'. A missing observation performs prediction
only. A nonpositive or numerically singular S is rejected before division;
there is no pseudoinverse, hidden jitter or silent skipped update.

Input is ASCII with mandatory LF, at most eight observations:

```text
C 1 0 1
P +1
M
V -1
P +3
```

The header is exactly `C d q r\n`: d is 1..3; q and r are 0..3.
An observation is exactly `P +n\n`, `P -n\n`, `V +n\n`, `V -n\n`, or
`M\n`, with n a digit 0..9. Empty streams, headers without observations,
unknown commands, invalid signs/digits, CRLF, partial lines and oversized
lines reject with exit 64. A ninth complete observation exits 66. Singular
updates exit 65, failed numerical invariants exit 67, and success exits 0.
Input storage is a fixed 16-byte buffer. This bounded demo intentionally
rejects multi-digit and fractional measurements; the state computation is f64.

Output begins with a field legend. Each `TRACE` row contains the step and
then position, velocity, p00, p01, p10, p11, innovation and innovation
variance, all as signed integers in millionths, followed by `update` or
`missing`. `missing` means no observation update was committed; the final
singular-error trace also uses this policy. Missing observations emit zero
innovation and variance. Output is an artifact suitable for redirection:

```sh
OMEGA=/absolute/path/to/omega
SAMPLE=samples/cli/independent/bounded_state_estimation
"$OMEGA" update --project "$SAMPLE" --target linux_x86_64
# Read the produced package review, accept its entry binding, then:
"$OMEGA" update --project "$SAMPLE" --resume
"$OMEGA" --check --target linux_x86_64 "$SAMPLE/main.omg"
"$OMEGA" run --keep "$SAMPLE/main.omg" < "$SAMPLE/data/missing_middle.txt" > trace.txt
"$OMEGA" run --both --keep "$SAMPLE/main.omg" < "$SAMPLE/data/velocity_correction.txt"
python3 "$SAMPLE/validate.py" --omega "$OMEGA"
"$OMEGA" --check "$SAMPLE/intended_invalid/invalid_index.omg"
```

The last command must reject an out-of-bounds covariance index, independently
of runtime errors. This is a complete separately built intended-invalid project.
Validation retains commands, exits, stdout, stderr and binary/oracle hashes in
`build/validation/`. Both native and Terminal comparisons are run sequentially.
The documented `--both` interface checks termination agreement; it does not
compare interpreter trace bytes. The validator requires explicit agreement,
rejects declines, and checks the surfaced native output against the oracle.

`expected.json` was frozen before Omega execution. `reference.py --freeze`
uses exact rational Gaussian conditioning P - (P h)(h' P)/S, independently
of the application's Joseph matrix implementation. Validation never regenerates
expectations. Fixtures cover constant velocity, missing observations, velocity
correction with dt=2, noisy observations, eight maximum-noise missing steps,
perfect measurements, singular updates, capacity and malformed runtime input.
Every expected row retains its exact fractions and fixture hash. Output tolerance
is 2e-6 absolute plus 1e-9 relative; this covers decimal truncation. Internal
symmetry, diagonal and determinant checks allow 1e-10; S must exceed 1e-12.
Printed PSD checks account for quantization error in the determinant rather
than requiring a rounded covariance to be exactly PSD. No failures are accepted
by widening tolerances after execution.

Semantic overlap was inspected before authoring: `collections/matrix_multiply`
computes one fixed integer product; `arithmetic/stats_compute` computes moments;
`simulation/particle_sim` integrates deterministic motion. None performs
observation conditioning, innovation accounting, covariance propagation,
missing-data policy or singular-update rejection. This sample adds that workflow.

Entrypoint and streaming control are in `main.omg`; `domain/matrix.omg` owns
four-entry matrix operations and covariance validation; `domain/filter.omg`
owns the update policy; `io/input.omg` and `io/trace.omg` own input/output.
Source and expected data have no generated-build dependencies. Maximum dt,
noise and record count also keep the finite demo's scaled covariance values
within the signed 64-bit formatter range (worst all-missing p00 = 4381).

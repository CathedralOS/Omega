# Ordinary kriging

Predict a latent spatial field with an unknown constant mean. The declared
process covariance is `C(p,q) = variance * exp(-||p-q|| / range)` in planar
coordinate units. Observation errors are independent with common variance
`noise`, added only to the training covariance diagonal. Query uncertainty is
for the latent field, without an added target observation error.

Input consists of whitespace-separated finite decimal tokens:

```
observation_count query_count process_variance range observation_noise_variance
x y observed_value             # one row per observation
query_x query_y                 # one row per query
```

Supply 2–8 observations and 1–8 queries, positive process variance and range,
and nonnegative noise variance. The reader accepts ordinary signed decimal
notation with at most 12 digits total and 8 fractional digits per token; it
rejects exponent notation, NaN, infinity, malformed tokens, trailing records,
and streams reaching the 2048-byte cap. Comments are not part of the protocol.
All records and EOF are required. The sample never waits for unbounded input
in its validation runs.

Each query emits a `query` row containing prediction, latent variance,
componentwise scaled system residual, and absolute unbiasedness residual;
its following `weights` row follows input observation order. Numeric tokens
are decimal scientific values with sixteen significant digits. The initial
`status` line identifies successful ordinary kriging. Exit 0 is success,
64 is invalid input, and 2 reports `status singular-system`: a normalized
augmented-system pivot has magnitude at most 1e-12. This last status includes
numerically unresolved nearly singular systems; no jitter or pseudoinverse
silently replaces the declared model.

The solver divides covariance by process variance and solves

```
[K / variance    1] [w     ] = [c / variance]
[1^T             0] [lambda]   [1           ]
prediction = w^T observed_values
latent_variance = variance * (1 - w^T (c / variance) - lambda)
```

Partial-pivot Gauss–Jordan elimination handles the indefinite mean constraint.
A residual is recomputed from the untouched training matrix and cross vector,
not from the eliminated matrix. Its componentwise denominator is
`1 + sum(abs(K_ij / variance * w_j)) + abs(lambda) + abs(c_i / variance)`;
the reported system residual is the maximum over training rows. The last
unbiasedness row is reported separately as `abs(sum(w)-1)`.

The local exponential routine reduces its nonnegative argument to at most
1/8, evaluates sixteen Taylor terms, and repeatedly squares. Arguments above
64 return zero (absolute covariance-correlation error below 1.7e-28). It does
not change the declared exponential model at the acceptance precision.

From the repository root, activate the host's normal development setup, build
an optimized producer, and retain its Cargo JSON artifacts:

```
mbx build --release -p omega --message-format=json > build/w2-24/cargo.json
omega update --project samples/cli/independent/ordinary_kriging --target linux_x86_64
# Resolve the exact generated package-policy choices before execution.
omega run samples/cli/independent/ordinary_kriging/main.omg < samples/cli/independent/ordinary_kriging/data/planar.stdin
python samples/cli/independent/ordinary_kriging/validation/validate.py --omega target/release/omega --route all
```

`validate.py --route all` links the public runtime sponsor against the exact
CLI-build artifacts, checks their binary hashes, prepares the admitted project,
compiles once, publishes native output, and executes its retained canonical
Terminal artifact with the report's actual runtime provisions. It captures and
parses fresh Terminal stdout, stderr and completion status, independently runs
the published native executable, and checks both numerical streams against the
frozen oracle. Exit agreement alone cannot pass. Before execution, the sponsor records the
retained host roster and refuses operations outside Console input/output/exit
and pure numeric realization. It supplies no clock or filesystem authority.
Native runs have a 30-second
limit; the sponsor has a 100-million-fuel allowance per case and the whole
observer invocation has a 30-minute wall-clock limit. Declines, fuel exhaustion,
partial execution, wrong refusal identities, and nonfinite output fail.

`validation/oracle.py` was run before application execution to freeze
`frozen.json` (SHA-256 in `FROZEN.sha256` and independently pinned in the
validator). It uses 80-digit Decimal exponential covariances and forward
elimination/back substitution of the physical covariance block system, with
analytic two-point weights and constant-field assertions. The Omega solver
uses normalized covariance and Gauss–Jordan, providing a distinct derivation.
Acceptance requires weights/predictions/variances within 1e-7, weight sums
within 1e-9, and scaled residuals at most 1e-9. Validation also reconstructs
latent mean-square error and tests permutation/translation invariance,
noiseless interpolation, noisy duplicates, maximum dimensions, far queries,
EOF/whitespace variants, singular duplicates, and malformed input.

`controls/covariance_index` is a source-invalid fixed covariance index (64 for
an array of extent 64). It must reject for its bounds obligation, never for
package policy or custody. Compiler write-frame witnesses live in the ordinary
`tests/omega/{pass,fail}/borrows` corpus, rather than becoming application modes.

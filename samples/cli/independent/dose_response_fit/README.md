# Dose-response fit

A synthetic Omega console application reads dose, replicate and control observations,
fits a bounded four-parameter logistic curve, and reports its half-dose, Hill slope,
asymptotes, predictions, residuals, SSE, convergence and profile intervals.
The application owns its parser, bounded storage and numerical computation.

The model is `prediction = A + (B-A)/(1+exp(-h*(log(dose)-t)))`.
`A` is the low-dose limit, `B` the high-dose limit, `exp(t)` the half-dose,
and `h` the positive Hill magnitude. Activation (`B>A`) reports EC50;
inhibition (`B<A`) reports IC50. Half-dose means halfway between the fitted
asymptotes, not an absolute response of 50. Limits are `A,B in [0,100]`,
`h in [0.2,5]`, and half-dose between one tenth the smallest sample dose and
ten times the largest. A control with role 1 observes A directly; role 2
observes B directly. Each replicate is one independently weighted observation.

Stdin begins with three signed decimal integers: direction (`1` activation or
`-1` inhibition), observation count (`8..32`), and known Gaussian response-noise
standard deviation in thousandths (`1..10000`). Exactly that many rows follow:
`dose replicate role response`. Sample doses are integers `1..10000`; role 0
means sample. Controls use dose 0 and role 1 or 2. Replicate identifiers are
`1..32`; repeated dose/role/replicate identities are rejected. Responses are
integer thousandths in `[-20000,120000]`. At least five distinct sample doses
are required. Whitespace may be space, tab, CR or LF; EOF must follow the final
row. Malformed, excess, truncated, oversized and duplicate records exit 2.
The input limit is 4096 bytes, including trailing whitespace.

The solver groups identical dose/role observations with Welford sufficient
statistics. It preserves all replicate weight and within-group SSE, and retains
each original row for prediction/residual reporting. It eliminates A and B
using bounded linear least squares, considering the interior and all four box
faces. An 81-point scan seeds the full shape fit; a 15-point scan plus the full
fit seeds two-dimensional nuisance fits. Damped reduced Gauss-Newton uses the
Schur complement of currently free linear asymptotes. Convergence requires both
projected gradients divided by their reduced diagonal curvature to be below
1e-7 (curvature floor 1e-12), within 100 iterations and 20 damping trials per
iteration. Stagnation alone does not count as convergence. Fixing either shape
parameter leaves a scalar nuisance problem: 17 scan points bracket a 32-step
golden-section minimization, which must finish with width below 1e-6.
Direct asymptote controls can rigorously exclude a profile point when their
own residual contribution exceeds the cutoff, without claiming a nuisance fit.
The private exponential divides by 128, sums 16 Taylor terms and squares seven
times; validated arguments have magnitude below 70. Logarithms normalize
positive doses and use an atanh series. These are bounded private evaluators.

Nominal 95% pointwise profile-likelihood intervals use the supplied known sigma:
`profile_SSE <= best_SSE + chi2(0.95,1)*sigma^2`. Each parameter is fixed in turn
while all remaining parameters are refit, followed by 23 bisection evaluations.
The chi-square calibration is asymptotic; it does not promise exact finite-sample
coverage. Endpoints must cross the cutoff strictly inside the parameter bounds.
A bound alone never establishes identifiability. Flat amplitudes at most five
sigma exit 3; open profiles or failed nuisance fits exit 4; wrong requested
direction exits 5; exhausted full-fit convergence exits 6. Accepted fits exit 0.
Numerical search is bounded and cannot certify a global optimum for arbitrary
observations. Independent multistart calculations challenge the supplied fixtures.

`validation/oracle.json` was frozen before application source or execution,
SHA-256 `9ec778992f7f88bdf7282a3d245b52f6c3d901a4121162d7cc2fc9f757a96187`.
Its independent reference directly fits all four variables with SciPy bounded
trust-region reflective least squares from 15 starts. It fixes one parameter,
refits three nuisance variables, and finds profile roots with Brent's method.
Noisy fixtures use deterministic perturbations of scale 0.7; the supplied
profile model treats sigma 1 as known, and does not estimate it from residuals.
The two exact fixtures have half-dose 9 and zero mathematical residuals; two
noisy fixtures challenge different directions and slopes. An additional independently frozen case uses unequal replicate counts and sigma
1.7 (`validation/oracle_groups.json`). The oracle records
all parameters, residuals and eight profile endpoints for each fixture.
`validation/reference.py` reproduces the independent calculations; `--freeze`
refuses to overwrite an existing oracle. `validation/validate.py` checks actual
reports against the frozen numerical expectations. Parameter/residual tolerance
is 2e-4, profile-endpoint tolerance 2e-3, and SSE tolerance 1e-5.

From the repository root, using a recorded compiler matching this library:

```sh
omega update --project samples/cli/independent/dose_response_fit --target linux_x86_64
# Inspect exact pending package decisions, then resume only authorized choices.
omega update --resume --project samples/cli/independent/dose_response_fit
python3 samples/cli/independent/dose_response_fit/validation/validate.py --omega /path/to/omega --route check
python3 samples/cli/independent/dose_response_fit/validation/validate.py --omega /path/to/omega --route native
python3 samples/cli/independent/dose_response_fit/validation/validate.py --omega /path/to/omega --route invalid
# Build the recorded producer once, retaining its exact artifact manifest:
mbx build --locked -p omega --release -j4 --message-format=json > /path/to/artifacts.jsonl
python3 samples/cli/independent/dose_response_fit/validation/producer_receipt.py \
  --manifest /path/to/artifacts.jsonl --producer-release /path/to/producer/target/release \
  --commit <full-sha> --output /path/to/exact-build-artifact-receipt.json
# Capture genuine Terminal output:
python3 samples/cli/independent/dose_response_fit/validation/terminal_validate.py \
  --producer-release /path/to/producer/target/release --producer-commit <full-sha> \
  --producer-receipt /path/to/exact-build-artifact-receipt.json
```

The intended-invalid control independently reads index 32 of a 32-element array.
It must reject for that exact bounds obligation. Empty stdin supplies the ordinary
sample gate's authored exit 2. Full scientific acceptance includes `terminal_validate.py`, which compiles the
actual accepted application through the existing public complete-project API,
then runs its retained canonical artifact with its exact runtime provisions.
Each fresh isolated invocation gets finite input, captured stdout/stderr and an
explicit sponsor budget (default 100 million units). The standard 10-million
CLI allowance can be insufficient for all repeated nuisance fits; exhaustion
is a decline and never counted as acceptance. The observer has an outer
wall-clock bound and checks the same frozen numerical/refusal expectations.
It supplies no filesystem permission or reconstructed artifact.
`omega run --both` compares termination only and
cannot establish numerical stdout agreement. A declined Terminal run is not acceptance.

References: [SciPy bounded least squares](https://docs.scipy.org/doc/scipy/reference/generated/scipy.optimize.least_squares.html),
[chi-square distribution](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.chi2.html),
[profile likelihood definition](https://www.mathworks.com/help/stats/nonlinearmodel.profilelikelihood.html).

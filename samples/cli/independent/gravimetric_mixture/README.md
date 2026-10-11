# Gravimetric mixture (IND20)

A complete offline two-constituent gravimetric verification application. Read
synthetic gross measurements and tare, correct each mass, propagate the declared
measurement uncertainty including reused-tare covariance, and accept only when
the entire expanded fraction interval lies inside the specification. All domain
code and fixtures are private to this project. The only dependency is stable std;
there is no dependency on another worker or an in-progress numeric package.

From the repository root, set `OMEGA_LIBRARY_ROOT` to this checkout's
`source/library` and use a compiler built from the recorded base:

```sh
omega update --project samples/cli/independent/gravimetric_mixture --target linux_x86_64
# Review the generated build/package-manager/review-linux_x86_64.txt decisions; accept the std dependency
# and its ordinary entry/provider grants, then:
omega update --resume --project samples/cli/independent/gravimetric_mixture
omega --check samples/cli/independent/gravimetric_mixture/main.omg
omega run --both --keep samples/cli/independent/gravimetric_mixture/main.omg \
  < samples/cli/independent/gravimetric_mixture/data/nominal.txt
python3 samples/cli/independent/gravimetric_mixture/validate.py --omega /absolute/path/to/omega
```

The invalid source is a separate complete project with its own entry and build
metadata. Prepare its std dependency the same way, then check
`invalid/main.omg`; it must reject the negative value assigned to `Nonnegative`.
The validator checks that rejection separately from invalid runtime inputs.

## Input and output

Exactly 14 bytes, including LF: `g003g005g0011\n`. Each reading consists of
one unit byte and three decimal digits: gross A, gross B, then tare. All three
units must match: `g` for grams or `m` for milligrams. The final model digit is
`1` for one shared tare measurement, `2` for independent tare measurements with
identical observed values. The application reads only one bounded record. EOF
without LF, extra bytes before LF, bad digits, unknown/mixed units and unknown
models reject. It does not prompt or pause. Digits encode nonnegative readings
from 0 through 999; negative *corrected* mass is a separate rejection. Exactly
0.01 g total is admissible; smaller totals reject before division.

Numeric output is computed and printed, not a fixture-specific success message.
Masses use integer micrograms; mass covariance and variance use integer billionths of g²;
fractions and standard/expanded uncertainty use integer ppm; fraction variance
uses integer billionths (`nano`). Signed interval endpoints are supported. Each
value is rounded to nearest displayed unit. Exits: 0 accept, 1 guard-band reject,
2 malformed record, 3 negative correction, 4 low total, 5 units, 6 model.

## Model and frozen independent oracles

Corrected masses are A=G_A-t and B=G_B-t after conversion to grams. Standard
uncertainties are 0.1 input units for each gross and 0.2 input units for each
tare. Gross observations are independent. Each corrected mass variance is
u_g²+u_t²; shared-tare covariance is u_t², independent-tare covariance is zero.
Total variance is V_A+V_B+2C. For f=A/T, the variance is
(B²V_A+A²V_B-2ABC)/T⁴. Both fractions share the same standard uncertainty.
Expansion factor k=2. Acceptance requires the complete fraction-A interval
f ± 2u_f inside 1/3 ± 0.04. This is first-order propagation under a declared
synthetic model; it is not an experimental confidence guarantee. There is no
buoyancy, drift, temperature or density correction in this offline scope.

`data/expected.json` was frozen before the first Omega execution. It contains
exact rational mass, composition and variance values plus 70-digit Decimal
square-root reference outputs. The independent `reference.py` uses derivatives
with respect to raw gross/tare observations, rather than the corrected-mass
covariance expression in Omega. It cannot serve as the application.
All numeric output tolerances were fixed to one displayed unit: 1 µg for masses,
1e-9 g² for mass variances, 1 ppm for fractions/uncertainties, 1e-9 for fraction
variance. Neither source nor reference expectations change to accommodate runs.

Nominal A=2 g, B=4 g has fractions 1/3 and 2/3, shared covariance 1/25 g²,
total variance 9/50 g², fraction variance 1/3600 and u_f=1/60; it accepts.
With independent tares, fraction variance is 1/1296 and total variance is 1/10;
the expanded interval rejects. Equal masses isolate common-tare cancellation:
shared variance 1/3200 versus independent 1/640. Milligram readings check unit
conversion and uncertainty scaling. Other variants cover off-target composition,
large/maximum readings, a zero component, the exact mass threshold, near-zero
and zero totals, negative correction, malformed records, and invalid units/model.

`validate.py` checks all 20 frozen fixtures and independent reference values.
`verify_domain.omg` additionally checks 13 domain cases (including invalid mass
controls) during Omega execution against all frozen numeric metrics. Its
bounded comparisons use the same fixed display tolerances; status 0 requires all
comparisons to hold. `render_domain_checks.py` reproducibly renders this
supplemental test from the checksum-verified frozen JSON. The production
application still reads measurements and emits computed evidence independently.

The validator runs one Omega process at a time with `run --both`, which executes native then
replays the exact produced Terminal. Because the CLI promises only termination
comparison, the validator independently verifies native numeric stdout and every
status; it also requires the CLI's explicit comparison agreement. The supplemental
domain entry therefore checks numeric values in Terminal as well as native. Raw stdout,
stderr, commands and exits are retained in ignored `build/validation/`.

## Semantic overlap review

Before authoring, samples were searched for mixture, tare, gravimetry,
uncertainty and covariance. None implemented this workflow. `stats_compute`
provides mean/variance/weighted sums; `vector_distance` provides scalar float
arithmetic/sqrt; `calculator` provides bounded byte parsing. This application
adds reused-observation covariance, mass/composition transformation, unit-aware
uncertainty scaling, guarded division and a composition guard-band decision.

# qPCR standard curve

Fits ordinary unweighted least squares to Cq = intercept + slope*log10(copies).
Standards represent a calibrated amplification assay. Residuals are observed
minus fitted Cq; R² = 1 - residual sum of squares / centered Cq sum of squares.
Amplification efficiency is 10^(-1/slope)-1 (1 means ideal doubling).
Unknown copies = 10^((Cq-intercept)/slope). A nonnegative Cq measurement margin
m yields [10^((Cq+m-intercept)/slope), 10^((Cq-m-intercept)/slope)]. These intervals
condition on the fitted calibration; they omit calibration uncertainty and
biological variation and are not confidence intervals.

Input: standard count (3–8), unknown count (0–8), that many copies/Cq standard
pairs, then that many unknown Cq/margin pairs. Copies must be in [1,1e6]; Cq in
[0,50]. Whitespace separates finite signed fixed decimals (up to 320 total digits,
17 fractional digits, mandatory digit before/after a decimal point). No exponents,
NaN or infinity. Accepted input contains at most 4096 bytes, including whitespace. A single
EOF lookahead may read byte 4097 solely to reject excess input.

Output lines are `name=significand@exponent`, meaning significand*10^exponent.
This decimal scientific transport retains twelve significant fractional digits
without saturating an integer conversion for large or tiny concentrations.
Residuals and unknown groups follow input order. `extrapolated` flags a point
outside the standards' log-copy span; `interval_extrapolated` flags an interval
endpoint outside it. Boundaries include a 1e-12 log-copy-unit rounding tolerance;
this prevents inversion roundoff from labeling a fitted endpoint extrapolated.
Extrapolation is reported, not clamped.

Exit 0: completed report. Exit 10: malformed/range input. Exit 11: rank-deficient
log-copy design or nonnegative slope. Exit 12: binary64 numerical representation
limit (nonfinite fitted coefficients or required natural exponent outside
[-700,700], including efficiency above exponent 700). An exit-12 partial report
is diagnostic output, not a successful quantification.

From the repository root, with a compiler built from the qualification revision:

```
omega update --project samples/cli/independent/qpcr_standard_curve --target linux_x86_64
omega run --both --keep samples/cli/independent/qpcr_standard_curve/main.omg < samples/cli/independent/qpcr_standard_curve/input.txt
python samples/cli/independent/qpcr_standard_curve/validation/validate.py --omega /absolute/path/to/omega --route all
```

Build binds the exact Application::main entry for Linux x86-64, Windows x86-64,
and macOS arm64. Qualification records the actual host; bindings alone establish
no execution result on other hosts. The validator verifies actual native stdout
and retained Terminal stdout against frozen 80-digit Decimal regression/inversion
oracles, with 1e-6 relative concentration and 1e-8 absolute coefficient/residual
bounds; efficiency uses 1e-8 relative to max(1,abs(value)). It checks 11 numerical
cases, 40 runtime refusals, exact byte/digit limits, extreme concentrations,
analytic doubling and invalid standard
index access. The application-private log/exp series do not claim a full math
library; reduction bounds and finite numerical limits are explicit.

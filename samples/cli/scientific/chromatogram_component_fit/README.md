# Chromatogram component fit

A complete synthetic chromatogram separation workflow, implemented in Omega.
It accepts two supplied detector-response profiles and five baseline-corrected
observations, fits nonnegative component amounts, and emits component areas,
a reconstructed trace, residuals, SSE and an identifiability warning.
It owns its parser, fixed storage, numerical algorithm and decimal reporting.
There are no numerics-library imports or dependencies on worker candidates.

The declared scientific model is a linear detector response at five times
0, 1, 2, 3, 4. Each profile's scale is supplied rather than normalized implicitly.
Amounts are nonnegative multipliers; areas are amount times trapezoidal profile
integral with unit time spacing. The fit minimizes unweighted squared error.
These synthetic fixtures demonstrate the model, not chemical identification,
instrument calibration, baseline estimation or chromatography physics.

Provide exactly 15 signed integers separated by space, tab, LF or CR: five profile A values,
five profile B values, then five observations. Each token has one to four decimal digits and an optional minus sign.
Magnitudes must be at most 1000;
profiles must be nonnegative and each must have nonzero norm. Observations may
be negative after baseline correction. Input uses fixed storage and a byte
budget of 512 bytes (one lookahead detects exhaustion). Malformed, truncated, extra, oversized, negative
profile and exhausted-budget input exits 2; a zero profile exits 3. Successful
fits exit 0. Before parsing, the program writes a two-sample baseline and a
two-sample peak through mutable windows of one four-sample scratch. The peak
window is the field of the record the selector returns. A failed window check
exits 5. No interactive pause or hidden fixture selector is used.

The private algorithm enumerates {}, {A}, {B}, {A,B}. Single-component solutions
use projection, the full solution uses a 2x2 normal equation, and infeasible
negative solutions are discarded. Each candidate must satisfy the exact KKT sign conditions of its active set.
Admissible candidates are scored through actual sample reconstruction. Gram
numerators are exact bounded integers, so even an objective improvement below
an SSE floating-point ULP retains its positive component. The full active set requires
both coefficients positive; a zero coefficient already belongs to a smaller
active set. Each Gram product is an integer at most 25,000,000,000,000, below
2^53, so its f64 determinant is exact. Only a zero determinant marks
nonidentifiability. Proportional profiles use A alone as the earliest nonempty
representative, without comparing floating point A/B ties. A nonidentifiable
report does not claim its amounts are uniquely identified.

`validation/reference.py` independently uses exact rational Gaussian elimination
and exhaustive active-set enumeration. `validation/oracle.json` preserves the initial oracle frozen before
Omega check or execution: SHA-256
`9f714012c2ac23ac921907be868458f71513fae07e3f096b156353f479f3ec33`.
Independent review before numerical execution exposed near-collinear and tiny
positive-component cases. `validation/oracle_v2.json` adds those exact rational
oracles and the corrected exact rank/selection policy, frozen at SHA-256
`654c9b334b07237630004572ff0ac6d12f7fd7a30d8e72e7f0198f8f4cf1bb30`.
`validation/oracle_v3.json` adds a positive component whose exact SSE gain is
below one floating-point ULP, frozen at SHA-256
`7aa66bebbc97a34ff7e00004d43973d9025b6c74fc0499407578dab37ea119e5`. `sub_ulp_derivation.json` records its integer derivation.
All earlier expected case records remain unchanged.
Reported numbers have six decimals. Output validation uses absolute tolerance
2e-6 plus relative tolerance 1e-9, checks every amount, area, trace bin, residual,
SSE, mask and warning, and independently checks nonnegative KKT conditions.

Fixtures cover exact overlapping mixtures, either absent component, noisy
interior and constrained-boundary fits, proportional profiles, zero and negative
signals, different supplied profiles, maximum observations, uniquely identifiable
near-collinear profiles and positive components with SSE gains below 1e-9 and below one SSE ULP. Invalid fixtures
exercise runtime parsing and profile checks. `controls/intended_invalid` is a
separate complete application that reads index 5 of a five-element array, violating [collection bounds](../../../../wiki/spec/language/numeric_values.md#collection-bounds); its
check must reject specifically for the documented array bounds obligation.

Semantic overlap was checked on base `7192503a18f205b359c279dd79575835c2b3a7bb`.
`arithmetic/vector_distance` supplies scalar magnitude only; `dual_float` checks
float widths; `algorithms/maze_flood` supplies bounded traversal and console
rendering; `text/format_number` supplies digit conversion. None fits supplied
profiles, enforces a nonnegative active set, assesses identifiability or reports
a reconstructed scientific signal. This adds that substantive workflow.
The sample scan was repeated on integrated main base
`d4b100d0aca0787dd0a003a1ae96a7f97e2ec1e4`; no chromatogram or NNLS application was present.
The final scan on main `e406db0cb06974bb4f3ba05fe0d257a61ee00460` also found no semantic overlap.

Run from the repository root, with a verified compiler matching its library:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
omega update --project samples/cli/independent/chromatogram_component_fit --target linux_x86_64
# Read the generated package review and record each exact acceptance; then resume.
omega update --resume --project samples/cli/independent/chromatogram_component_fit
python3 samples/cli/independent/chromatogram_component_fit/validation/validate.py --omega /path/to/omega --route check
python3 samples/cli/independent/chromatogram_component_fit/validation/validate.py --omega /path/to/omega --route native
python3 samples/cli/independent/chromatogram_component_fit/validation/validate.py --omega /path/to/omega --route both
# Prepare the separate control's dependency review in the same documented way.
python3 samples/cli/independent/chromatogram_component_fit/validation/validate.py --omega /path/to/omega --route invalid
```

The ordinary native sample gate supplies EOF and checks the declared exit 2
input rejection. The full scientific acceptance uses the fixture-aware validator.
The complete invalid control uses `invalid.omg` so ordinary valid-sample discovery
does not classify an intentional rejection as a valid application.

Native reports and captured outputs live under ignored `build/validation`.
The CLI's Terminal comparison checks termination, not stdout equality; this
validator checks the native report's full numerical output and requires an
explicit comparison agreement. A Terminal decline is recorded as unsupported,
never counted as a passing comparison. Only Linux x86-64 is bound and validated.

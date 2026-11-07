# Crossed gauge study (NEW014)

A complete stdin-driven crossed measurement study, with private bounded integer
algorithms and real console output. This explicitly declared sample model has two
parts, two operators, and two repeat observations per cell. The random-effects
model is `y = grand + part + operator + part:operator + repeat error`. It is an
educational method-of-moments decomposition, not a measurement-system certification.
No external numerics implementation, another sample, or another worker is used.

Input is a `2` header followed by eight single digits in `0..9` or `m` (missing),
in part/operator/repeat order, then EOF. Spaces, CR, and LF are ignored. The bound
keeps the maximum intermediate below 21,000; signed i32 arithmetic never wraps.
`1` reports insufficient replication immediately. Missing cells and partial cells
are refused rather than fed into balanced-design formulas. Other headers, invalid
bytes, extra observations, and premature EOF exit 2. Missing cells exit 3, unequal
replication exits 4, insufficient replication exits 5. Valid studies exit 0.

All report numbers are exact numerators over **32**, including sums of squares,
mean square error, signed raw component estimates and individually clamped
variance components. ANOVA degrees of freedom are 1,1,1,4, total 7. The interaction
is retained; no significance test or pooling is claimed. Gauge variance sums
repeatability, operator variance, and interaction variance. Negative raw estimates
are visible so clamping cannot hide them. Zero variation is identifiable but has
zero components; ratios requiring positive total variation are deliberately absent.

For each effect, SS is its balanced contrast squared divided by 8. Error SS is
half the sum of four paired differences squared. Components use
`error/4`, `(MSinteraction-MSerror)/2`, `(MSpart-MSinteraction)/4`, and
`(MSoperator-MSinteraction)/4`. The separate Fraction reference computes marginal
means and residual sums, and directly computes total SS. It does not use these
contrast identities.

Before the first Omega check/run, `python3 reference.py --freeze` froze
`data/expected.json`; SHA256
`8ca6770dce8b8cc15b470aaad8f41eeacb6a1cec0d7202e4b92a738a486c6832`.
Tolerance is **zero** for rational numerators, stdout bytes and exits. Fixtures
cover additive effects, interaction, pure interaction with negative main effects,
zero variation, translation and label/repeat permutations, and invalid input.
The closest existing sample, `arithmetic/stats_compute`, computes mean/variance
of one fixed series and a weighted sum; it has no crossed cell validation,
interaction decomposition, signed components or stdin study workflow.

From the repository root, using a compiler built from the recorded base:

```sh
omega update --project samples/cli/independent/crossed_gauge_study
# Read the generated review; accept only requested std/entry decisions and resume.
omega update --resume --project samples/cli/independent/crossed_gauge_study
omega --check samples/cli/independent/crossed_gauge_study/main.omg
omega samples/cli/independent/crossed_gauge_study/main.omg
omega run --both samples/cli/independent/crossed_gauge_study/main.omg < samples/cli/independent/crossed_gauge_study/data/interaction.stdin
python3 samples/cli/independent/crossed_gauge_study/validate.py --omega /absolute/omega --native /absolute/published-executable
omega --check samples/cli/independent/crossed_gauge_study/controls/out_of_bounds/main.omg
```

The final command must reject specifically because index 8 is outside `[i32;8]`;
package, entry, parser or type failures do not count. The invalid source is a
separate complete project and never imported by the application. `build/` holds
compiler and execution artifacts and is ignored. Validation passed on the recorded Linux x86-64 compiler: source check and native
compilation, all 19 exact-output native fixtures, all eight native/Terminal exit
comparisons, and the separate bounds rejection control. The CLI comparison
observes Terminal exits; interpreted stdout is not exposed by this route and is
not claimed as verified by that CLI route. The documented repository corpus
runner additionally passed all 19 native and admitted Terminal stdout, stderr
and exit oracles, using unchanged copies of the complete application and frozen
fixtures. That historical corpus matcher allows CRLF-to-LF normalization;
separate native captures use raw-byte equality. The final review bundle records
an additional private test-only observer of actual native and Terminal buffers
for the frozen zero-byte tolerance, without changing the ordinary matcher or runtime. See `EVIDENCE.md` for commands, producer/binary identity,
package-review continuity, artifacts and validation scope.
Only Linux x86-64 is declared here; other native targets are untested.

# Age structured population (NEW057)

A complete synthetic census forecast in Omega. Source checking has reached
package admission; native and Terminal execution are currently blocked by a
denied bundled-std package acceptance. See `EVIDENCE.md` for the exact boundary. Three bounded integer cohorts
(newborn, juvenile, open-ended adult) evolve under supplied cohort-specific
survival, fertility and signed migration rates. This is a declared sample model,
not an empirical demographic prediction. No external scientific library or
worker candidate is imported.

Input is exactly 13 signed decimal fields, one per LF-delimited line, ending
at EOF: horizon; newborn, juvenile, adult; three survival rates; three fertility
rates; three migration counts. Rates are thousandths in [0,1000], population
counts are [0,1000000], migration is [-1000000,1000000], horizon is [0,32].
At most ten decimal digits per field are accepted. Empty, malformed, missing,
overlong and trailing inputs exit 2 before any census is printed.

For old cohort counts n0,n1,n2, survivor counts are floor(ni*si/1000).
Births are floor((n0*f0+n1*f1+n2*f2)/1000), computed from the old census.
The next census is [births+m0, survivors0+m1, survivors1+survivors2+m2].
Deaths equal old total minus all survivors. The oldest cohort remains open
ended. Migration is applied after reproduction and survival; migrants do not
reproduce in their arrival step. Rounding removes fractional expected survivors
and births according to these explicit rules. All accepted computations stay
within signed 64-bit range; the application rejects a next cohort outside the
population bound with exit 3. It emits only feasible rows, followed by the error.
Every published row checks new total = old total + births - deaths + net migration.
Zero-horizon runs emit only the initial census. Valid forecasts exit 0.
The ordinary sample gate supplies empty stdin, so its annotations require the
correct missing-input rejection (exit 2); the 23-case validator exercises forecasts.

`demography/model.omg` owns the private census recurrence and admission rules;
`demography/oracles.omg` checks 17 transitions from six pre-frozen fixtures at startup;
`demography/input.omg` owns the bounded streaming parser; `demography/report.omg` owns
signed decimal CSV reporting. `main.omg` reads the input and drives the forecast.
`build.omg` binds the entry to the documented Linux x86-64 program slot and uses
the existing std Console provider. Output is stdout CSV suitable for redirection.

Existing samples were compared before authoring: `borrowed_sum_population`
exercises linear values and borrowed population of a case carrier, rather than
population dynamics; `particle_sim` advances a scalar Euler trajectory;
`elevator` and `random_walk` model commands or spatial motion. None implements
age progression, an open terminal cohort, fertility schedules or census balances.

## Frozen acceptance

`reference.py` independently derives census rows with Python integers and exact
`Fraction` rates. It freezes expected results before any Omega execution.
`fixtures/oracle.json` records every input and output SHA-256 and required exit.
Numerical tolerance is exactly zero. `validate.py` pins the frozen manifest/reference identities and all 23 cases,
then verifies every fixture before execution. It compares every stdout byte and
exit status, requiring empty native stderr or the exact documented CLI exit
diagnostics for Terminal comparison; it never rewrites expectations.
Twenty-three cases cover births, extinction, signed migration, fractional rounding,
zero horizon, the open terminal cohort, maximum horizon, invalid survival and
fertility, negative rates/counts, horizon/migration bounds, infeasible emigration,
capacity overflow, negative net migration, reproduction before deaths, ten-digit
integer admission, malformed/missing/empty/overlong/trailing input.

The complete separate control `controls/invalid_rate_type` violates the documented
integer-versus-boolean type rule by assigning `true` to an `i64` rate field.
It must fail specifically for that mismatch. Package admission, parse failure,
provider failure or any other diagnostic is not a successful negative control.

## Commands

From the repository root, use a compiler built from the recorded base revision
with its matching `source/library` (set `OMEGA_LIBRARY_ROOT` to that absolute path).
Here `omega` names that compiler and `S` is this sample directory:

```sh
S=samples/cli/independent/age_structured_population
omega update --project "$S" --target linux_x86_64
# Review the exact generated pending decisions before accepting and resuming.
omega --check "$S/main.omg"
omega --build-dir "$S/build" "$S/main.omg"
omega run --both --keep "$S/main.omg" < "$S/fixtures/births.input"
python3 "$S/validate.py" /absolute/path/to/the/published/executable
python3 "$S/validate.py" /absolute/path/to/omega --both
omega --check "$S/controls/invalid_rate_type/invalid.omg"
```

Do not run `reference.py --freeze` during validation. Native runs compare exact bytes with the frozen output oracle. Terminal replay
uses the same input; the current documented `--both` CLI compares exit status,
so that route establishes exit agreement and does not attest Terminal stdout.
Startup oracle checks make exit agreement also attest 17 independently frozen
transitions (all three cohorts and births/deaths/migration/before/after values).
An internal oracle disagreement exits 4, before reading any user input. The intended-invalid
control is checked separately, without executing it. Validation evidence and
precise supported/denied routes are recorded in `EVIDENCE.md`; generated compiler
artifacts and actual stdout/stderr remain under ignored `build/`.

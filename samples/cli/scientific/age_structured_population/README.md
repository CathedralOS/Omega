# Age structured population (NEW057)

The census forecast runs on an ARM64 Mac. Across all 23 frozen inputs, the
native executable and the Mac-target Psi interpreter produced the expected
exit status and exact CSV or error message. Three bounded integer cohorts (newborn,
juvenile, open-ended adult) evolve under supplied cohort-specific survival,
fertility and signed migration rates. This is a declared sample model, not an
empirical demographic prediction. No external scientific library or worker
candidate is imported.

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

`demography/model/model.omg` owns the private census recurrence and admission rules;
`demography/oracles/oracles.omg` checks 17 transitions from six pre-frozen fixtures at startup;
`demography/input/input.omg` owns the bounded streaming parser; `demography/report/report.omg` owns
signed decimal CSV reporting. `main.omg` reads the input and drives the forecast.
`build.omg` binds the entry for Linux x86-64 and ARM64 macOS. The Build grants
standard input, standard output and process exit to the bundled system library. Output is stdout CSV suitable for redirection.

Existing samples were compared before authoring: `borrowed_sum_population`
exercises linear values and borrowed population of a case carrier, rather than
population dynamics; `particle_sim` advances a scalar Euler trajectory;
`elevator` and `random_walk` model commands or spatial motion. None implements
age progression, an open terminal cohort, fertility schedules or census balances.

## Frozen acceptance

`fixtures/oracle.json` records every input and output SHA-256 and required exit.
Numerical tolerance is exactly zero. Twenty-three cases cover births, extinction,
signed migration, fractional rounding, zero horizon, the open terminal cohort,
maximum horizon, invalid survival and fertility, negative rates/counts,
horizon/migration bounds, infeasible emigration, capacity overflow, negative net
migration, reproduction before deaths, ten-digit integer admission,
malformed/missing/empty/overlong/trailing input.

The complete separate control `controls/invalid_rate_type` violates the documented
integer-versus-boolean type rule by assigning `true` to an `i64` rate field.
It must fail specifically for that mismatch. Package admission, parse failure,
provider failure or any other diagnostic is not a successful negative control.

## Commands

From this sample directory on an ARM64 Mac, use the compiler with its matching
`source/library`. Use `target=linux_x86_64` on a Linux x86-64 host.

```sh
target=macos_arm64
set --
for source in build.omg main.omg demography/*/*.omg; do
    set -- "$@" --build-input "$source"
done
omega packages update --project . --target "$target" "$@"
# After reviewing the generated package decisions, resume the update.
omega packages update --project . --resume
omega run --target "$target" "$@" --input fixtures/births.input --outputs build/psi build.omg
omega --target "$target" "$@" --build-dir build/native build.omg
mbx reserve --cpus 1 -- build/native/omega-program < fixtures/births.input
omega --check --target "$target" --build-input build.omg --build-input invalid.omg controls/invalid_rate_type/build.omg
```

Native runs compare exact bytes with the frozen output oracle. The recorded comparison replayed the same input
in Psi and compared termination, stdout and stderr with
the native run. Startup oracle checks make exit
agreement also attest 17 independently frozen transitions (all three cohorts and
births/deaths/migration/before/after values). An internal oracle disagreement exits 4,
before reading any user input. The intended-invalid control is checked separately, without
executing it.

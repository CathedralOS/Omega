# Thermocouple compensation

This complete Omega CLI reads supplied calibration and observation data, compensates
cold-junction voltage, enumerates segment brackets, and inverts one unique root.
It uses a declared sample measurement model, not a certified thermocouple standard:
`E(T) = a + b*T + c*T²`, with Celsius temperature and millivolt voltage.
There are no dependencies on other workers or an unfinished numerics package.
The private model, parser, inversion, and report modules live in `domain/`.
The only package dependency is the existing standard console/entry provider.

Input is exactly 15 whitespace-separated signed decimal integer tokens, scaled
by 0.001. Two segment rows each contain `low high a b c`; the observation row
contains `measured_voltage cold_temperature reference_temperature bracket_low
bracket_high`. Each token has at most seven digits and magnitude at most
1,000,000. The complete stream must be shorter than 512 bytes; one slot is reserved for EOF. Signs require a digit;
extra tokens and truncated or malformed streams reject. Input ends at EOF. Public input helpers declare Console invocation, blocking and
possible host-read Trap; callers explicitly acknowledge blocking.

Each segment must lie in [-100, 200] C, have increasing temperature endpoints,
`|a| <= 100`, `|b| <= 1`, `|c| <= 0.01`, and a strictly positive or strictly
negative derivative at both endpoints. The two temperature domains are contiguous
and their voltages agree within 1e-8 mV at the shared endpoint. These checks
establish segment monotonicity, while permitting global turning curves. The cold
junction must be covered, and the requested bracket must be increasing and inside
the complete calibration domain. Compensation is `measured_voltage + E(cold)`.
Each segment is clipped to that bracket and checked for endpoint enclosure.
Endpoint signals within 1e-12 mV snap to the corresponding endpoint to absorb
floating-point roundoff. Forty bisection steps invert every interior enclosing segment. Roots within 1e-8 C are
merged at the common segment boundary; other multiple roots are ambiguous.

Success emits `brackets`, `temperature_mC`, signed `error_mC` relative to the supplied
reference, `compensated_nV`, and signed `residual_nV`, then `OK`. Numeric fields
truncate toward zero. Exit 0 means one root; errors are 10 input, 11 calibration,
12 cold junction, 13 invalid bracket, 14 no root, and 15 ambiguous roots. Failure
reports never claim a recovered temperature.

Existing samples were compared before authoring: `basics/temperature_convert`
only performs a unit conversion; `algorithms/horner_eval` evaluates an integer
polynomial at one argument; `arithmetic/stats_compute` computes summary statistics.
None accepts piecewise calibration, cold-junction observations, clips and enumerates
inversion brackets, or distinguishes absent and ambiguous roots. This workflow
combines these new operations and runtime validation rather than renaming a kernel.

`reference.py` independently evaluates expanded polynomials with 70-digit Decimal
arithmetic and solves quadratic roots analytically. `expected.json` and its fixture
hashes were frozen before the first Omega check or run. Tolerances are 2 milli-C
for temperature/error, 2 nV for compensated voltage, and 1 nV for residual; statuses,
bracket counts and process exits are exact. The maximum numerical bisection
interval error is 300/2^40 C; output truncation dominates the report tolerance.
The oracle contains 23 cases including compensation changes, both endpoints,
segment boundary deduplication, decreasing curves, clipped/invalid brackets,
ambiguous roots, invalid calibration and malformed/over-budget input.

From the repository root, with a compiler built from the same revision/library:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
OMEGA=/absolute/path/to/verified/omega
PROJECT=samples/cli/independent/thermocouple_compensation
"$OMEGA" update --project "$PROJECT" --target linux_x86_64
# Review the generated decisions through the documented package review workflow.
# Resume only after those decisions are accepted; retain any denial as a blocker.
"$OMEGA" update --resume --project "$PROJECT"
"$OMEGA" --check --target linux_x86_64 "$PROJECT/main.omg"
"$OMEGA" run --both --keep --target linux_x86_64 "$PROJECT/main.omg" < "$PROJECT/data/curved_hot.txt"
"$OMEGA" inspect-terminal --machine Application::main --target linux_x86_64 "$PROJECT/main.omg"
"$OMEGA" --check --target linux_x86_64 "$PROJECT/controls/invalid_bounds/control.omg"
```

The intended-invalid control is a complete separate entrypoint reading array
index 2 from a two-element array. Its check must reject for the exact bounds
obligation in `controls/invalid_bounds/expected.txt`; unrelated package, parser,
type or entry errors do not pass that control. Each project may require its own
accepted entry review. Native/Terminal evidence and exact commands belong in
`VALIDATION.md`; generated outputs remain under ignored `build/`.

The serial validator records exact commands, exit status, elapsed time and captured
output under `build/validation/`. It consumes the frozen fixture hashes and refuses
a rewritten oracle. It never updates package acceptance. After authorized review:

```sh
python3 "$PROJECT/validate.py" --omega "$OMEGA" --route check
python3 "$PROJECT/validate.py" --omega "$OMEGA" --route native
python3 "$PROJECT/validate.py" --omega "$OMEGA" --route both --cases curved_hot,hot_boundary,decreasing_unique,ambiguous,reversed_bracket,malformed
python3 "$PROJECT/validate.py" --omega "$OMEGA" --route invalid
python3 "$PROJECT/validate.py" --omega "$OMEGA" --route terminal
```

The `--both` comparison establishes Terminal/native process-exit agreement. The
independent numeric/output oracle checks the captured native reports separately;
exit agreement is not a proof of equivalent intermediate calculations.

# Piston cycle work (NEW029)

A complete bounded stage runner using declared scientific sample models.
Pressure is in kPa and volume in litres; their product is joules. Positive work
is work by the piston. This is a quasi-static imposed pressure path, not a gas
constitutive law. Constant-volume stages interpolate imposed pressure and do
zero work; they do not simulate heat transfer or infer temperature.

`domain/model.omg` owns stage validation, pressure and analytic integration.
`data/fixtures.omg` owns the declared inventory. `domain/report.omg` prints
computed six-place numbers. `main.omg` owns input, stage execution, continuity,
pressure-volume histories and cycle work accounting.
`validation/terminal.rs` uses the existing compiler/package-manager API to run
the produced Terminal artifact and capture its actual output. The private
`build_terminal.py` links it against the single verified compiler build; the
harness does not compute reference physics or manufacture compiler artifacts. There are no numerics or
worker dependencies. The existing dual-float, particle-simulation, temperature
conversion and heat-grid samples share scalar arithmetic or state traversal,
but none integrates a pressure path or verifies a closed thermodynamic cycle.

Provide exactly one ASCII digit followed by EOF, or LF then EOF. Input consumes
at most three bytes. Empty input, other digits, extra bytes and long records
exit 64. Interactive use requires EOF after the selection. All fixtures declare
at most four stages; volumes must lie in [0.25,8] and starting pressure in
(0,64]. Exponent 1 is deliberately outside this integer-power analytic model
because its work uses a logarithm. The selectable inventory keeps pressure at
most 12 kPa and absolute stage work at most 8 J, so the fixed-point decimal
conversion fits i32. Reporting honestly publishes its Trapping conversion
envelope; valid fixture executions must return normally and match the oracle.

| Input | Declared path | Expected work (J) |
|---|---|---:|
| 0 | Rectangle: 1→2 L at 4 kPa; hold/cool; 2→1 L at 1 kPa; hold/heat | 3 |
| 1 | Reverse of the rectangle | -3 |
| 2 | n=2, 12 kPa, 1→3 L | 8 |
| 3 | n=3, 8 kPa, 1→2 L | 3 |
| 4 | n=-1, 2 kPa, 1→3 L | 8 |
| 5 | Zero end volume | reject, exit 65 |
| 6 | Rectangle with discontinuous pressure before compression | reject, exit 66 |
| 7 | Unsupported exponent 1 | reject, exit 65 |
| 8 | Stationary n=2 stage, 4 kPa at 1 L | 0 |
| 9 | Reverse n=2 stage, 4/3 kPa, 3→1 L | -8 |

Each stage prints start, midpoint and endpoint `(volume,pressure)` rows and its
computed work. The balance prints positive, negative and net work. Closed cycles
must return to their starting pressure and volume, and every run verifies net
work equals the signed component sum. Open stages report `open_stage` explicitly.
Continuity and accounting use absolute tolerance 1e-9. Output uses 2e-6 absolute
tolerance because it is rounded to six decimal places.

The independent rational reference uses `P=P0*(V0/V)^n` and
`W=(P1*V1-P0*V0)/(1-n)`. For holds it uses W=0. `validation/oracle.py` freezes
these results into `expected.json`; this file is produced before any Omega run.
`validation/verify.py LOG_DIRECTORY ROUTE` checks every numerical row, stage
work, component balance, exit, closure and invalid-input diagnostic. It never
learns expected values from application output. The separately frozen
`input_cases.tsv` exercises each digit with LF-plus-EOF and digit-plus-EOF,
plus five invalid input records including an extra byte after LF. The intended-invalid control is
a complete separate source project delivering nominal `Pressure` to a `Volume` parameter;
only the documented type-mismatch diagnostic is acceptable.

Commands are run from the repository root with `OMEGA_LIBRARY_ROOT` selecting
this checkout's `source/library`. See `validation/RESULTS.md` for binary identity,
exact commands and observed routes. Package preparation uses normal reviewed
`omega update --project samples/cli/independent/piston_cycle_work --target linux_x86_64`,
then `update --resume` after accepting the concrete required decisions. Denied
acceptance is a blocker and must not be retried via another route.

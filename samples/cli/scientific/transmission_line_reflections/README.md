# Transmission line reflections

A complete Omega console application for a declared scientific sample model:
a lossless, nondispersive, two-section transmission line with positive integral
one-way delays. Voltages are sampled rectangular travelling pulses, one second
per tick, not a continuous electromagnetic field solver. The load is a linear
resistive boundary. The source port comprises a passive resistive reflection
followed by an ideal travelling-wave injector; open and short are exact limiting reflection
coefficients. No numerical library under development is imported.

Input is one ASCII line `CASE HORIZON\n`, where CASE is one digit 0..5 and HORIZON
is one or two digits 1..24 without leading zero. EOF before LF, malformed digits,
negative values, unsupported cases and out-of-range horizons exit 64. One request
is consumed; subsequent lines are outside the single-request protocol.

| Case | Z1/Z2 ohm | Source/load reflection | Delays | Excitation |
|---|---|---|---|---|
| 0 | 50/50 | 0/0 | 1/1 | +1 V at ticks 0,1 |
| 1 | 50/100 | 1/2,0 | 1/2 | +1 V at ticks 0,1 |
| 2 | 50/100 | 1/2,1 (open) | 1/1 | +1 V at ticks 0,1 |
| 3 | 50/100 | 1/2,-1 (short) | 1/1 | +1 V at ticks 0,1 |
| 4 | 100/50 | -1/2,1/2 | 2/3 | +1 V at ticks 0,1 |
| 5 | 50/100 | 1/2,1 (open) | 1/1 | +1 V at 0,1; -1 V at 4,5 |

Finite boundary resistance is `R=Z(1+r)/(1-r)`. The source emits its prescribed
excitation plus the reflected arriving voltage. Case 5 exercises excitation
interfering with returning waves, exercising the coherent source-work cross term. Four private
64-slot calendars schedule all outgoing waves; each slot contains the coherent
voltage, not an independently squared branch energy. Capacity guards are ordinary
Omega bounds checks. The maximum written tick is 26; no queue growth is possible.

For simultaneous incident voltages `a` from section 1 and `b` from section 2,
`r=(Z2-Z1)/(Z2+Z1)`, outgoing voltages are `c=r*a+(1-r)*b` to section 1 and
`d=(1+r)*a-r*b` to section 2. They satisfy `a+c=b+d` and
`(a-c)/Z1=(d-b)/Z2`. Packet energy is `v*v/Z` joules for the one-second samples.
The injector work includes the coherent cross term:
`((r_source*s+drive)^2-(r_source*s)^2)/Z1`. Heat is the incoming energy less
reflected energy at each passive boundary. Source heat measures return-wave
absorption before injection, rather than the total heat of a driven Thevenin
resistor; the explicitly declared port model defines these energy records.
The running ledger subtracts all arrivals,
adds all emissions, and retains energy still travelling beyond the horizon.
Every tick checks `abs(work-source_heat-load_heat-travelling)<=1e-9` J.

Output has a version header, three CSV records per tick, and `END`. Signed values
are decimal integers in micro-units (truncation toward zero, twelve digit fields).

* `W,t,a,b,c,d,source_in,source_out,load_in,load_out`: wave-event voltages.
* `V,t,source_voltage,junction_voltage,load_voltage`: terminal voltages.
* `E,t,work,source_heat,load_heat,travelling,residual`: cumulative energies.

`physics/model.omg` owns the complete algorithm and fixtures; `io/input.omg` owns
bounded parsing; `io/report.omg` owns formatting; `main.omg` owns execution.
The checked core float operators and the ordinary std Console provider are the
only facilities used. Build metadata binds the exact `main` entry to the five
hosted targets. Execution validation is scoped to the available Linux x86_64 host.

The repository comparison covered all existing samples with searches for
reflection, impedance, transmission and pulse, then inspected `particle_sim`,
`bouncing_ball`, `generic_ring_buffer`, `alarm_scheduler` and `dual_float`.
Those cover kinematics, wall bounces, storage, dispatch and arithmetic. None
implements delayed impedance scattering or a coherent wave energy ledger.

`reference.py` independently expands signed rational packet branches, merges
physical arrivals, verifies junction voltage/current and energy identities, and
rescans future packet groups for stored energy. It does not share Omega's running
ledger algorithm. `data/oracle.json`, the six input fixtures and CSV projections
were frozen before the first Omega check/run. `data/freeze.json` binds SHA256
identities and the immutable absolute tolerance: 2 micro-units per numeric field,
1 microjoule for residuals. All exact expected residuals are zero. `validate.py`
checks header, terminator, complete record count, ordering and every field.

Independent hand checks: case 0 ends with 0.04 J load heat and no travelling
energy from tick 3 onward. Case 2 has load voltage 8/3 V at tick 2; at tick 3,
simultaneous arrivals `a=1/6,b=4/3` give `c=17/18,d=-2/9`, junction voltage
10/9 V. Case 3 has identically zero load voltage and load heat. These features
are present in the frozen fractions, not inferred from Omega execution.

Use current main's compiler, including its authenticated IEEE contract lowering.
`evidence/validation.json` records the original protocol;
the delivery bundle distinguishes historical and integrated execution receipts.
From repository root, with `OMEGA` set to that executable:

```sh
export PYTHONDONTWRITEBYTECODE=1
"$OMEGA" update --project samples/cli/independent/transmission_line_reflections --target linux_x86_64
# If update opens a pending review, inspect it and use ordinary review decisions.
# Resume only that pending review:
# "$OMEGA" update --resume --project samples/cli/independent/transmission_line_reflections
"$OMEGA" --check --target linux_x86_64 samples/cli/independent/transmission_line_reflections/main.omg
"$OMEGA" run samples/cli/independent/transmission_line_reflections/main.omg < samples/cli/independent/transmission_line_reflections/data/case_2.stdin
"$OMEGA" run --both samples/cli/independent/transmission_line_reflections/main.omg < samples/cli/independent/transmission_line_reflections/data/case_2.stdin
"$OMEGA" --check samples/cli/independent/transmission_line_reflections/controls/invalid_bounds/main.omg
python3 samples/cli/independent/transmission_line_reflections/validate.py 2 24 OUTPUT_LOG
```

The last compiler command must reject selector 64 into a 64-element calendar
with `cannot prove index`. This is a separate complete intended-invalid source;
import, parsing, custody or package-review rejection is a wrong reason and fails
the control. `build/` contains local generated products and is ignored. Evidence
and the delivery bundle preserve exact commands, exits, source hashes and output
artifacts. Package acceptance denied by the executor is recorded without retry.

For the full matrix, `run_validation.py --route native --omega COMPILER
--executable VERIFIED_NATIVE --executable-sha256 HASH` executes one native
artifact sequentially for all 18 valid variants and 11 malformed inputs.
`run_validation.py --route both --omega COMPILER` compiles once with
`omega run --both --inputs` over all 29 inputs, then checks every native and
Terminal `.stdout` with `validate.py` and each engine's exit independently. This
validates full Terminal records rather than treating the CLI's exit comparison
as output equivalence.
The exact exercised invocations and any denied routes are in the evidence record.

Three complete reproductions at the recorded baseline justified bounded repairs:
`controls/float_state_repro` for IEEE state predicates, and the complete scattering
fixture `tests/omega/pass/float/coherent_field_energy_scattering_exit` for named
requirements inside constructors; and
`tests/omega/pass/float/live_division_operand_across_call_exit` for a fixed-register
dividend that remains live across a call and a loop back-edge. Instruction
selection gives the x86-64 saturating division its own dividend copy. Current
main supplies generalized constructor traversal, fixed-register operand
isolation and the shared IEEE contract owner. This sample adds no compiler
implementation changes. The original witnesses and regression controls remain complete.
The application retains its authored contracts,
ordinary providers and independent oracle across those repairs. The compiler
regressions also exercise f32, NaN/infinity, strict comparisons, unguarded arrival
rejection, and constructor result policies for overflow versus division by zero.

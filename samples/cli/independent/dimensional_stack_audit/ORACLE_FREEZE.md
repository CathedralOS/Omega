The independent reference was executed before the first Omega check, native or
Terminal run. Its data/oracle.json SHA-256 is:
4e637f25b6173b516e75fa860e89ef31d502f08cd545f04fbc95da390ab7bec3

All result tolerances are exactly zero micrometres (relative tolerance zero).
The physical and measurement model is explicitly an authored linear sample
model. The reference enumerates all 64 joint endpoint vertices, including
independent local tolerances and shared temperature, gauge_a and gauge_b.
The compiler result will never be used to regenerate expectations.

Default [-10,20] temperature intervals:
1: [0,0], guaranteed; 2: [19,31], guaranteed; 3: [9,51], conditional;
4: [46,84], impossible; 5: [5,47], guaranteed; 6: [-6,6], conditional.
Additional thermal variants and all offending-contributor lines are frozen in
data/*.stdout and data/oracle.json. Invalid inputs have exit 64 and exact output.

The invalid source must reject specifically the call
nonnegative_tolerance(-1), whose documented requires value >= 0 is false.
Package-review refusal, syntax rejection, unrelated diagnostics or an incomplete
proof verdict do not satisfy this control.

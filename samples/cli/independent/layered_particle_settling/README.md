# Layered particle settling (NEW030)

A complete bounded console application simulates a particle under declared
buoyancy and drag laws. The implementation and root search live here; it imports
only the existing core and standard console packages, with no worker or numerics
library dependencies. Input selects a workflow and particle density at runtime.

This is a nondimensional scientific sample model, not a calibrated prediction.
Particle volume and gravitational acceleration are both one. Positive depth and
velocity point down. Mass equals particle density. Net downward force is
`density - fluid_density - c*v` for linear drag and
`density - fluid_density - c*v*abs(v)` for quadratic drag. All drag coefficients
are positive; reversing velocity reverses drag. Neutral buoyancy cancels weight
and buoyancy, but moving neutral particles still experience drag.

| Layer | Depth | Fluid density | Drag law/coefficient |
|---|---|---:|---|
| 0 | depth < 0.5 | 2 | linear, 2 |
| 1 | 0.5 <= depth < 2 | 3 | quadratic, 1 |
| 2 | depth >= 2 | 4 | linear, 4 |

Each semi-implicit step samples the initial layer, computes force, updates
`v_next=v+dt*force/mass`, then `depth_next=depth+dt*v_next`. A crossing changes
the next step's law. The sample does not interpolate interface crossing times.
At depth -8 or +8 it clamps depth and ends the trace; its last velocity is the
pre-contact velocity, and it models no restitution or boundary impulse. Step
force residual is `mass*(stored_v_next-v)/dt-force`, checked before clamping.
The maximum simulated duration is four time units. Refinement is an explicitly
different discrete scheme, not an asserted equality to the coarse trajectory.

Input is exactly `M 0D`, followed by LF, CRLF, or EOF: mode 1..6, density 1..8.
The 16-byte input buffer rejects full, malformed, empty and overlong lines.
Only the first line is the request; later lines are outside this one-shot
protocol. EOF never invents a default. Exit 0 means completed, 64 means invalid
input, and 70 means a residual or finite-envelope check failed.

| Mode | Workflow | Initial depth/velocity | Steps/dt |
|---|---|---|---|
| 1 | homogeneous layer 0 | 0 / 0 | 8 / 0.5 |
| 2 | layered settling | 0 / 0 | 8 / 0.5 |
| 3 | layered rise | 3 / 0 | 16 / 0.25 |
| 4 | moving neutral/relaxation variant | 0 / 1 | 8 / 0.5 |
| 5 | refined layered trace | 0 / 0 | 16 / 0.25 |
| 6 | bounded bottom contact | 7.75 / 2 | 8 / 0.5, early stop |

Output starts with model/input rows, then three independent terminal force
balances. Linear terminal speed is drive/c; a private 40-iteration bisection
finds signed quadratic terminal speed in [-8,8]. `REGIME`, `NEUTRAL`, `CROSS`
and `BOUND` rows expose changes. `TRACE` columns are step, initial layer,
ending depth, ending velocity, initial net force, inertial residual. Depth,
speed, force and residual are signed integer microunits truncated toward zero;
step/layer are unscaled signed integers. No formatted floating output package
is needed. Fixed signed width preserves zero and negative values.

The oracle was independently frozen before Omega execution. `data/reference.py`
uses exact rational arithmetic for every discrete trace, and analytic square
roots for terminal balances. `data/frozen.json` and `frozen.sha256` preserve it.
Trace output permits one microunit for floating rounding/truncation boundaries;
force residual checks permit absolute 1e-8. Bisection's speed bracket after 40
halvings is at most 8/2^40, supporting the frozen analytic terminal speed
tolerance of 1e-8. The validator compares printed terminal speeds only at
microunit precision; it does not measure that tighter raw speed tolerance.
The raw speed bound relies on the bisection interval and runtime force checks.
A printed zero alone cannot establish the force tolerance: the application
checks the unquantized residual at runtime. Event rows, layer choices, counts, exits and
invalid messages must match exactly. The validator never recalculates expected
results, performs acceptance, or accepts a wrong-reason control rejection.

`particle_sim` only evolves one constant-acceleration particle for five Euler
steps. `bouncing_particles` and `bouncing_ball_2d` cover grid/wall reflection and
rendering. None provides buoyancy, density validation, changing fluid drag,
terminal root search, force residuals or layered bidirectional crossings.

From this repository root, with a matching current compiler at OMEGA_BINARY:

```sh
OMEGA_LIBRARY_ROOT="$PWD/source/library" "$OMEGA_BINARY" update --project samples/cli/independent/layered_particle_settling --target linux_x86_64
# Read the emitted package review and accept only its reviewed decisions.
# Resume once if approved; do not retry a denied acceptance.
OMEGA_LIBRARY_ROOT="$PWD/source/library" "$OMEGA_BINARY" update --resume --project samples/cli/independent/layered_particle_settling
python3 samples/cli/independent/layered_particle_settling/validate.py --omega "$OMEGA_BINARY" --route check
python3 samples/cli/independent/layered_particle_settling/validate.py --omega "$OMEGA_BINARY" --route invalid-source
python3 samples/cli/independent/layered_particle_settling/validate.py --omega "$OMEGA_BINARY" --route native
python3 samples/cli/independent/layered_particle_settling/validate.py --omega "$OMEGA_BINARY" --route terminal
```

Each route runs one Omega operation at a time and records exact commands,
exits, binary/oracle hashes, stdout and stderr under local `build/<route>/`.
Native validation checks all trace rows for nine substantive fixtures, LF/CRLF/
EOF variants and nine invalid requests. Terminal uses the public `run --both`
route on those same requests; the CLI's comparison establishes termination
agreement and explicitly declines unsupported interpretation. Its exposed stdout
is native stdout, so this does not claim independent Terminal output agreement.
The separate complete `controls/missing_layer_bound` source intentionally indexes
three layers by an arbitrary runtime index without a guard or requirement. Its
expected diagnostic pins the documented index-bound obligation. A caller's known
constant does not prove the general machine valid.

Linux x86_64 is the validated target. Build metadata binds existing hosted
entrypoints on five targets; other native hosts require their own validation.
Package locks and generated output are local, excluded from the candidate.

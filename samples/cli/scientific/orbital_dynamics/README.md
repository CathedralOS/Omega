# Planar orbital dynamics

A test body moves around a fixed central mass under acceleration
`a = -mu r / |r|^3`. The body does not perturb that mass. The program uses
second-order velocity Verlet: a half kick, full drift and half kick. No force
softening or artificial energy correction is applied. Central kicks and straight
drifts preserve angular momentum to floating-point roundoff; energy oscillates
with the integration error.

Standard input contains exactly seven finite, fixed decimal fields:

```text
mu x y vx vy timestep steps
1 1 0 0 1 0.00490873852123405 1280
```

Spaces, tabs, LF and CR separate tokens; EOF completes the record. Decimal
fractions and a leading minus are supported. Exponent notation, NaN, infinity,
plus signs, malformed numbers, extra tokens, trailing junk and input requiring
more than 512 reads are rejected. The parser bounds its accumulator and decimal
scale before multiplication; it cannot create a nonfinite model input. At most
30 fractional digits are accepted. Stdin must be a finite stream.

`mu` must be in `(0, 1000000]`, each position and velocity component in
`[-1000000, 1000000]`, timestep in `(0, 1]`, and steps an integer in `1..4096`.
Zero radius is invalid. These bounds are a numerical envelope, not physical
units: callers must use a consistent unit system. Small timesteps are necessary
for accuracy, especially near eccentric periapsis. The main unit-orbit accuracy cases use
mu=1, semimajor axis=1 and circular timesteps no greater than 0.005. A circular
coarse convergence control uses approximately 0.009817; the general ODE case uses
mu=2. The coarse position tolerance is four times the unit-orbit tolerance.

CSV checkpoints include the initial state and every completed step: step, time,
x, y, vx, vy, specific energy `v²/2-mu/r`, signed angular momentum `x*vy-y*vx`,
and signed changes from the initial energy and momentum. Sixteen significant
digits are emitted as an integer mantissa and decimal exponent. `periapsis,time`
records interpolate an inward-to-outward zero crossing of `r dot v`; this is an
approximate event time, not an analytic extremum solver. Nearly circular
numerical radial oscillations may produce these records too.

A radius at or below `1e-6` produces `status,collision_threshold`, exit 2. Before
each full drift, the closest point on that straight segment is checked as well
as its endpoint, preventing a step from silently jumping through the central
mass. This is a conservative discrete-segment refusal, not a continuous Kepler
collision-time calculation. No singular force or diagnostics are evaluated
inside the threshold. Syntax refusals exit 64, model-range refusals exit 65,
reporting-envelope failure exits 66, and completed integration exits 0.

The frozen oracle was authored before the sample was compiled or executed.
`validation/reference.py` independently solves analytic circular/eccentric Kepler
orbits and the ODE using SciPy DOP853 at rtol `2.3e-14`, atol `2e-15`. Their
agreement is below `1e-10`. It never implements velocity Verlet or copies sample
output. `validation/frozen.sha256` pins the generator, oracle and the original 30 inputs.
An independently reviewed interior-crossing control was added before execution in
`validation/extra_frozen.sha256`, preserving every original frozen byte. Exact 30/31 digit and 511/512 byte
boundaries were subsequently frozen in `validation/parser_frozen.sha256`. Validation
consumes all three manifests (36 cases) and never regenerates them. SciPy is
needed only to regenerate references, not to run or validate the sample.

The checker compares every native and captured Terminal checkpoint to the ODE,
recomputes energy and momentum from reported coordinates, checks their drift
columns, validates eccentric periapsis against time pi, and verifies second-order
timestep-halving convergence. The unit circular final-position tolerance is
`1e-4`; eccentric position tolerance is `0.0015`, with velocity tolerance five
times the corresponding position tolerance. Angular momentum drift is bounded
by `1e-10`. Reversed, rotated, oblique, general, maximum-step and collision
trajectories accompany invalid syntax, radius, bounds and overflow controls.

With an exact qualified compiler, from the repository root:

```sh
project=samples/cli/independent/orbital_dynamics
omega update --project "$project" --target linux_x86_64
# Read the generated review; accept only the intended scoped decisions.
omega update --resume --project "$project"
omega --check "$project/main.omg"
omega run --both --keep "$project/main.omg" < "$project/data/circular.stdin"
python3 -B "$project/validation/run_native.py" /path/to/retained/executable /path/to/native-observations
python3 -B "$project/validation/link_terminal.py" /path/to/qualified-build.jsonl "$PWD" /path/to/terminal-observer
# This observer has its own executable identity: prepare its normal review first.
/path/to/terminal-observer --review "$project"
# Read the review; preserve existing scoped authority decisions; then resume.
/path/to/terminal-observer --resume-review "$project"
/path/to/terminal-observer "$project" /path/to/terminal-observations
python3 -B "$project/validation/check_output.py" /path/to/terminal-observations
```

Std source-risk disclosures do not authorize host access. Package preparation
and the Terminal observer retain the normal acceptance, executable identity,
provider, authority and custody checks. Denial is a blocker. `run --both` checks
termination parity when both routes complete. Its default reference fuel can be
exhausted by a complete one-period numerical report; that DECLINED result does
not establish parity. The public observer uses a bounded 200-million-unit fuel
allowance. Numerical acceptance comes from the actual captured outputs.
The observer executes the exact retained Terminal artifact using the ordinary
runtime provisions of the accepted native product, with finite stdin and bounded
fuel. It never substitutes source interpretation or reconstructed grants.

`controls/intended_invalid/main.omg` independently calls the same coordinate
bound contract with `1000001.0`. It must be rejected because that precondition is
disproved. This package-free control cannot pass on a package-review diagnostic.
The runtime parser's index and input-envelope guards remain separately checked.
The build explicitly binds `Main::main` for Linux x86-64, macOS ARM64/x86-64 and
Windows x86-64. Execution evidence is qualified on the actual host, not inferred
for other targets.

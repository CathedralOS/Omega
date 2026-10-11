# Constrained pendulum motion (NEW021)

This is a complete Linux x86-64 Omega application. Its private integrator and
numeric reporter use stable core float operations and the bundled std console.
There are no worker-produced numerics imports or shared library modifications.

The scientific sample model is a unit point mass on a massless rigid rod,
length L metres, under constant gravity 9.81 metres/second². The pivot is the
origin, y points upward, and the bob starts on the lower hemisphere. The
experiment runs 32 steps of 0.01 seconds, without damping or driving.

For q=(x,y), v=(vx,vy), differentiating q.q=L² twice gives q.a=-v.v.
With a=(0,-g)-lambda*q, lambda=(v.v-g*y)/L². Each step kicks the velocity,
drifts the position with the kicked velocity, normalizes q onto the circle,
and removes the radial velocity component. This first-order projection method
has numerical dissipation; the reported energy change quantifies it. This is
an explicitly declared model and bounded demonstration, not an exact nonlinear
pendulum solver or an energy-conserving integrator.

Input is exactly three bytes followed by EOF: scenario digit `0`..`3`, length
digit `1` or `2`, then LF. Waiting for finite console input uses the documented
`block` surface. Malformed, truncated, extra-byte and invalid-length inputs exit
64 before any integration or trajectory emission. At most four bytes are read.

| Scenario | Initial state |
| --- | --- |
| 0 | Equilibrium q=(0,-L), v=0 |
| 1 | Small angle x=0.01 L, y=-sqrt(L²-x²), v=0 |
| 2 | Released rest q=(0.6 L,-0.8 L), v=0 |
| 3 | Off-circle q=(0.66 L,-0.88 L), v=(0.3,0.4), projected before stepping |

Output contains 33 `ROW` records including the projected initial state. Columns
are step, x, y, vx, vy, pre-projection squared-length residual, post-projection
squared-length residual, q.v and energy. Every number is a signed integer in
micro-units (divide by 1,000,000); even the step column uses that encoding.
`ENERGY_CHANGE` compares the final energy with the projected initial energy.
Output is deliberately plain text so a redirected run is a reusable trajectory
artifact. `report/report.omg` owns the decimal writer. Saturating conversion is
explicit; authored cases stay below 1000 in magnitude.

`data/oracle.json` was independently frozen before compiler checks or executions.
`reference.py` derives the trajectory using 60-digit Decimal arithmetic; it
also checks x(t)=0.01 L cos(sqrt(g/L)t) and vx(t)=-0.01 L sqrt(g/L)
sin(sqrt(g/L)t) for both small-angle lengths. The 0.0002 position and 0.0001
velocity tolerances accommodate first-order time discretization. Output versus
discrete oracle has absolute tolerance 0.000002, covering six-decimal truncation
and native roundoff. The tolerance is never fitted to an Omega run.

Separate checks derive the first released-rest kick as v=(-0.047088,-0.035316)
and the unprojected q=(0.59952912,-0.80035316); normalizing gives the first
position. The initial off-circle projection has raw squared-length drift 0.21
and tangent velocity (0.384,0.288). Equilibrium remains exact.

The nearest samples were inspected before authoring: `simulation/particle_sim`
advances a free particle with Euler steps and `rendering/bouncing_ball_2d`
applies gravity and wall reflections. Neither enforces a rod-length equality,
projects tangent velocity, compares two analytic small-angle lengths, or reports
constraint and energy trajectories. This is a new constrained experiment.

From the repository root, using an exact verified compiler as `$OMEGA`:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
project=samples/cli/independent/constrained_pendulum_motion
"$OMEGA" update --project "$project" --target linux_x86_64
# Inspect omega.review and accept only the required decisions; resume once.
"$OMEGA" update --resume --project "$project"
"$OMEGA" --check "$project/main.omg"
"$OMEGA" run "$project/main.omg" < "$project/data/released.stdin"
"$OMEGA" run --both --keep "$project/main.omg" < "$project/data/small_double.stdin"
python3 "$project/validate.py" --omega "$OMEGA"
```

Package review/acceptance is explicit and host-local. Denied acceptance is a
blocker; do not retry it. `omega.lock` and generated `build/` are ignored.
`run --both` compares termination, so the validator also checks native stdout
against the frozen numeric oracle. Terminal agreement is not a stdout oracle.

`controls/invalid_bounds` is a separate complete intended-invalid application:
a fixed two-element carrier is selected at constant index 2. Its rejection must
name the out-of-bounds selection obligation. A missing package review, type error
or other wrong-reason rejection does not pass. It has its own build and review.

The candidate is prepared for independent review; publication must use the
main-only workflow in `tools/landing.md`. No remote worker branch is required.

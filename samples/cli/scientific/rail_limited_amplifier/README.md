# Rail limited amplifier (NEW034)

This complete Omega application reads a bounded circuit configuration from stdin,
computes the piecewise DC equilibria, and emits sampled output and feedback traces,
clipping states, final-phase settling and recovery metrics. It owns its parser,
scientific model and formatting. It imports only existing core/std facilities.

The scientific sample model is deliberately explicit: a finite open-loop gain A,
feedback fraction beta, discrete relaxation alpha, and output rails L < H give

```
f[n] = beta*y[n-1]
r[n] = y[n-1] + alpha*(A*(u[n]-f[n]) - y[n-1])
y[n] = min(H, max(L, r[n]))
q = 1 - alpha*(1 + A*beta)
```

Positive beta is negative feedback. This is a projected first-order Euler model;
it does not model slew rate, transistor physics, propagation delay or rail-dependent
pole shifts. Saturation is applied after the update. A bad timestep can therefore
produce a bounded rail-to-rail oscillation even with negative feedback. A stable
interior requires -1 < q < 1. Instability is reported and simulated, rather than
silently changing alpha. These are model assumptions, not hardware design advice.

The DC solver independently tests both rail inequalities and the strict interior
candidate A*u/(1+A*beta). Equality at a rail is represented once. Positive feedback
can produce three equilibria. When the denominator and drive both vanish there is
a continuum; division by zero is never evaluated. The continuum and all candidate
flags are reported. Settling is qualified by a unique equilibrium and -1 < q < 1;
a rail attractor in an unstable interior configuration does not qualify.

Input is one line of ten space/tab separated signed decimal integers:

```
gain feedback_per_1000 relaxation_per_1000 lower_mV upper_mV initial_drive_mV recovery_drive_mV switch_step steps initial_output_mV
4 500 250 -2000 3000 5000 250 4 20 0
```

Each integer has magnitude at most 10000; gain is 1..100, feedback -1000..1000,
relaxation 1..1000, steps 1..64 and switch 0..steps. Initial output must be inside
the rails. The line buffer has 192 bytes; Full is rejected, including a line without
LF that fills it. EOF after a shorter valid line is accepted. Partial tokens,
exponents, extra/missing fields and overflow are rejected. The first complete line
is the request; later lines are not consumed. Syntax, invalid rails and invalid
parameters print distinct errors and exit 64. Valid simulations exit 0.

Updates 1..switch_step use the initial drive; subsequent updates use the recovery
drive. Row zero is the supplied initial output, with initial drive and zero clipping.
The final-phase settling comparison begins at the switch boundary itself.

Output is newline-delimited CSV following one version/units line:

- `E,phase,lower_exists,interior_exists,upper_exists,continuum,interior_uv,q_ppm,stable`:
  phases 0 and 1 are initial/recovery drive. `interior_uv` is the algebraic candidate
  even when outside the rails, or zero if the denominator is zero.
- `T,step,drive_uv,output_uv,previous_feedback_uv,unprojected_output_uv,overload`:
  clipping is -1 below the lower rail, +1 above the upper rail, and zero otherwise.
  Exact equality at a rail is zero. Feedback is from the previous sample.
- `M,settled_from,clipped_update_count,first_unclipped_recovery_update`:
  `settled_from` is the first index whose entire observed final-phase suffix lies
  within 1 mV of the unique stable equilibrium, or -1. It describes the bounded
  observation horizon, not a proof about all future samples. Recovery is the first
  unclipped update strictly after the switch, or -1, even if no prior overload occurred.

Presentation truncates toward zero to integer microvolts (q to parts per million).
The independently frozen oracle allows 2 units of error in voltage/q columns only;
flags, metrics, row counts, exits and errors match exactly. Physical settling uses
an inclusive 1 mV band before presentation. Bounded inputs keep voltage presentation
within i64 range; the formatter's saturating conversion is a formatting policy.

`domain/model.omg` owns equilibrium and evolution, `io/input.omg` the bounded parser,
`io/output.omg` decimal output, and `main.omg` the complete console workflow. Build
metadata binds the ordinary Linux x86-64 ProgramEntry. Existing `particle_sim` only
updates position/velocity with constant acceleration; `stats_compute` computes static
statistics. Neither parses circuit parameters, solves piecewise equilibria, tracks
feedback, classifies overload or measures recovery. Repository searches found no
other amplifier workflow. This application is not a renamed existing sample.

`reference.py` uses exact rational affine maps and independently enumerated equilibria.
The authored fixture inventory and numerical tolerances were frozen before the
first Omega check/native/Terminal execution in `data/oracle-freeze.json`. Do not run
`freeze` to bless an execution mismatch. The verifier checks the reference and fixture
hashes. There are thirteen valid and fourteen invalid input cases, including negative
feedback, both rail recoveries, damped alternation, positive feedback reaching the
upper rail, unstable timestep, asymmetric rails, rail equality, continuum and
zero-denominator drift and positive/negative near-singular algebraic candidates. The
original 24-case freeze remains unchanged; `data/edge-oracle-freeze.json` separately
freezes the two near-singular cases and the 64-update parameter-endpoint case
before their execution. Expected files are reference products, never captured
Omega output. `controls/invalid_bounds` is a separate complete intended-invalid
application: its two-slot history is written at index two. It must reject for
`cannot prove index`; package-review failure, type failure or another reason is
not acceptance of that control.

Run from the repository root with one compiler/build/execution at a time:

```
source /workspace/cloud-setup/activate.sh
mbx build --release -p omega
./target/release/omega update --project samples/cli/independent/rail_limited_amplifier --target linux_x86_64
# Read the generated package review, accept only the reviewed decision rows, then:
./target/release/omega update --project samples/cli/independent/rail_limited_amplifier --resume
./target/release/omega --check samples/cli/independent/rail_limited_amplifier/main.omg
./target/release/omega run --both --keep samples/cli/independent/rail_limited_amplifier/main.omg < samples/cli/independent/rail_limited_amplifier/data/rail_recovery.in
```

Package acceptance is local and excluded from the candidate; a consuming executor
must perform its own review. If acceptance is denied, stop that route and report
the exact denial. Do not copy an accepted lock or bypass custody checks. A compiler
run requiring the supported real-ownership execution context can request ordinary
per-command sandbox escalation; that is independent of package acceptance.

The private `validation/terminal.rs` driver calls the existing complete accepted
project compilation operation, executes its produced admitted Terminal artifact with
finite fixture input and captures stdout/stderr/status, then publishes that same
native artifact. It uses neither a tree interpreter nor a synthetic exit. The normal
CLI `--both` compares exit alone; this driver supplies the separate Terminal trace
oracle. `validation/run.py` executes the published native image serially and checks
both streams against the unchanged frozen oracle. Build/link commands and observed
results are recorded in `VALIDATION.md` once executed. Generated artifacts belong
under ignored `build/`; a transfer bundle carries evidence separately.

Observed execution status: all 27 fixtures pass on both native and Terminal,
including 14 invalid runtime inputs. The separate complete bounds control rejects
only for its intended index proof failure. Package acceptance was explicitly
authorized and succeeded on the original project; the current-main compiler
refresh introduced zero new capability decisions. See [VALIDATION.md](VALIDATION.md)
for exact identities, commands, tolerances and transfer evidence.

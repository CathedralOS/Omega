# Disk collision lab

A complete console application for event-driven Newton disk collisions. It owns
its model, all-pairs event search, impulse calculation, fixtures, bounded input,
reporting and checks. It uses only the existing standard Console provider and
core IEEE f64 operations. No numerics package or worker output is imported.

Enter one ASCII digit and LF. Cases 0–6 exercise unequal-mass elastic head-on,
grazing, simultaneous independent contacts, separating disks, one-event
budget, inelastic restitution and oblique contact. Cases 7–9 intentionally
reject mass zero, initial overlap and restitution outside [0,1]. Empty input,
non-digits, extra characters before LF, missing LF and CRLF also reject.
The program reads one bounded command; later bytes belong to a later command
and are not consumed. There is no interactive pause.

The declared scientific model is isolated rigid circular disks with no spin,
friction, external forces or walls. Positions evolve linearly between impacts.
Newton restitution acts only along the unit contact normal. At most four disks,
eight events and ten seconds are admitted. Coordinates, velocities, masses and
radii have explicit finite bounds. NaNs fail the comparisons. All initial pairs
are checked for overlap. Momentum and energy-plus-dissipation are checked after
each event at absolute tolerance 1e-9.

The quadratic contact search rejects stationary and separating pairs, keeps
nonnegative approaching roots inside the remaining horizon, and recomputes
after each impulse. It treats relative speed squared <= 1e-12 and radial
products >= -1e-12 as inactive, and admits root slack of 1e-9 seconds before
clamping to zero. These are explicit numerical cutoffs. Exact simultaneous times use lexicographic disk indices.
The simultaneous fixture uses independent pairs. This sequential convention is
not a physical multi-body simultaneous-contact solver. Grazing events record
zero impulse once. Initial touching separating pairs are skipped. The budget
stops before a pending event and reports the last accepted state; it never
advances through an unresolved contact. Reaching exactly the budget with no
pending event still completes.

Contacts emit pair IDs, time, impulse and contact residual. Momentum/energy
rows include elapsed time, both momentum components, kinetic energy and
cumulative dissipated energy. Final disk rows include both position and
velocity components. Output is signed fixed point with six fractional digits.
Capture stdout as the ledger artifact. Exit 0 means completion, 2 invalid
runtime input/model, 3 exhausted event budget, 4 invariant violation.

Existing samples were compared before authoring: `rendering/bouncing_ball_2d`
is a fixed-step gravity/wall animation; `simulation/particle_sim` is a five-step
single-particle integrator; `arithmetic/vector_distance` computes one static
norm. None searches all disk pairs, orders contacts or applies and audits
Newton pair impulses. This is a distinct workflow.

`data/oracle.md`, `data/expected.json` and `data/FROZEN.sha256` freeze independent
rational reference calculations and tolerances before Omega execution.
`validate.py` checks output against those frozen values; it is not the program
or the delivered simulation. `data/expectations.omg` also tests the final
computed state against the frozen oracle before success, so Terminal exit
comparison checks the scientific results. Batch execution also retains each
engine's actual stdout and stderr, so the same frozen output oracle can check
both. Fixtures are actual stdin byte files.

From the repository root, with a current compiler `OMEGA`:

```sh
"$OMEGA" update --project samples/cli/independent/disk_collision_lab --target linux_x86_64
# Read and accept the exact pending package decisions through the documented
# review workflow, then use update --resume. Do not bypass denied acceptance.
"$OMEGA" --check --target linux_x86_64 samples/cli/independent/disk_collision_lab/main.omg
"$OMEGA" --target linux_x86_64 samples/cli/independent/disk_collision_lab/main.omg
"$OMEGA" run --both samples/cli/independent/disk_collision_lab/main.omg < samples/cli/independent/disk_collision_lab/data/case_0.stdin
"$OMEGA" --check samples/cli/independent/disk_collision_lab/controls/intended_invalid/main.omg
```

The separate complete intended-invalid source writes a two-entry contact ledger
then reads index 2. It must reject for the documented array bounds rule;
`controls/intended_invalid/expected.txt` pins `cannot prove index`. A package,
parser or missing-entry diagnostic is not a successful control. The positive
application guards all runtime indices before access.

Build metadata binds exact `Main::main` entry contracts for Linux x86-64,
Windows x86-64 and macOS arm64. Execution evidence here is host-specific;
other hosts require their own provider/package preparation and validation.
See `VALIDATION.md` for exact measured commands, revisions and remaining limits.
For a host-local replay of all fifteen frozen inputs with one compilation:

```sh
omega run --both --inputs samples/cli/independent/disk_collision_lab/data --outputs build/disk-collision-results samples/cli/independent/disk_collision_lab/main.omg
```

`results.tsv` records each engine's exit; `validate.py` checks every scientific
case's native and `.terminal.stdout` files against the same frozen physics.
Malformed-input cases must return 2 with the exact invalid-input message.
The historical Linux serial harness remains `python3 -B run_validation.py /absolute/omega`.
Generated child builds and Python bytecode are excluded from accepted source;
retained ledgers and exact commands are in `evidence/qualified/`.

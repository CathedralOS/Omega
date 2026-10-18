# Bounded resource scheduler (IND02)

This complete console application computes an optimal nonpreemptive schedule
for up to four tasks, two renewable resources and finish-to-start precedence.
It owns its model, parser, search, verifier and rendering. Its only product
dependency is the existing standard console provider; no active worker or
in-progress numerics library is needed.

Send one ASCII line on stdin:

```text
count capacity_A capacity_B duration_0 demand_0A demand_0B predecessors_0 ...
4 3 2 2 1 0 0 3 2 1 1 2 1 1 1 1 1 1 6
```

The first three fields are the task count (0–4) and capacities (0–4). Each
zero-based task contributes four fields: duration (1–5), demand on each
resource (0–4), and a predecessor mask. Bit `j` means task `j` must finish
before this task starts. Masks cannot name the task itself or an absent task.
Numbers are unsigned ASCII decimal, at most 99, separated by space, tab, CR or
LF. EOF may end a nonempty line. A full 256-byte buffer without LF is rejected;
only the first line is a request. Missing, extra, negative and malformed fields
are rejected. Zero tasks, including zero capacities, are valid.

The result lists starts and ends in task-ID order, then makespan:

```text
TASK 00 START 00 END 02
TASK 01 START 02 END 05
TASK 02 START 02 END 04
TASK 03 START 05 END 06
MAKESPAN 06
```

Intervals are half-open `[start,end)`. All results are integers: tolerance is
exactly zero. Equal optima choose the lexicographically smallest start vector
in task-ID order. Success exits 0; malformed/out-of-domain input prints
`INVALID` and exits 2; a cycle or resource demand exceeding its capacity prints
`IMPOSSIBLE` and exits 3. A failed internal invariant prints
`INVARIANT_FAILURE` and exits 4. There is no interactive closing pause.

`domain/solver.omg` enumerates 256 radix-four priority words, discarding repeated
IDs, nonexistent IDs and precedence-incompatible orders. For each legal order
it inserts each task at its earliest capacity-feasible time, checking every
occupied tick on both resources. Serial schedule generation spans the active
schedules. For a regular makespan objective an optimum can be made active by
left-shifting tasks; the lexicographically first optimum must also be active.
Thus this bounded search contains both the optimal makespan and its chosen tie.
The sum of durations is at most 20, and every feasible acyclic instance admits
a serial schedule within that horizon. `domain/verify.omg` separately recomputes
every duration, precedence inequality, per-tick capacity and maximum end before
any output is emitted. Wrapping integer carriers are private implementation
storage; admitted bounds exclude overflow in this algorithm.

Existing samples were compared before authoring: `simulation/alarm_scheduler`
is a scripted alarm-state dispatcher; `systems/task_runner` accumulates a score
through scripted task variants; `algorithms/maze_flood` computes BFS distances;
`algorithms/bubble_sort` sorts a fixed array. None has resource-capacity placement,
precedence-constrained optimal search or independent optimum verification.

From the repository root, using a compiler built from the same source tree:

```text
mbx build -p omega
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
OMEGA=target/debug/omega
$OMEGA update --project samples/cli/independent/resource_scheduler --target linux_x86_64
# Read the emitted review, accept the existing std entry/console requirements,
# and use update --resume if the update requests that review.
$OMEGA --check --target linux_x86_64 samples/cli/independent/resource_scheduler/main.omg
$OMEGA run --both --keep samples/cli/independent/resource_scheduler/main.omg < samples/cli/independent/resource_scheduler/data/diamond.input
```

Use the actual target directory if the build system selects another location.
The build binds the ordinary std program entry for Linux x86-64/arm64, macOS
x86-64/arm64 and Windows x86-64. This task validates Linux x86-64; other hosts
need their own executions. The commands above use a POSIX shell; on Windows
use the equivalent environment assignment and input redirection.

The top-level `input.txt` and exact stdout/stderr/exit files describe the diamond
request. The sample sweep feeds `input.txt` and scores those files; the main
header separately pins the correct `INVALID` / exit 2 response to an empty
manual request. The negative
control uses `invalid.omg` so ordinary sample discovery does not mistake it for
a passing `main.omg` project.

Input storage belongs to the private named `Input` record; state edges use
ordinary record loans. `validation/reproduction` retains the independently
witnessed bounded-byte state-loan compiler gap on the recorded main base.

The 53 input/output fixture pairs (41 distinct requests) and `data/expected.json` were independently
frozen before Omega execution. `validation/reference.py` exhaustively enumerates
integer start vectors up to the sum-of-duration horizon, rather than priority
orders or the application's placement algorithm. The frozen oracle examines
82,387 start vectors. It includes a greedy-order trap (optimum 9), parallel and
bottleneck diamonds, reversed task IDs, endpoint sharing, deterministic ties,
three-way capacity contention, zero-resource precedence, a 20-tick horizon, zero demands/capacities, zero tasks, cycles, excess demand,
24 deterministic variants and invalid inputs. `reference.py` is a deliberate
oracle-authoring tool; acceptance never runs its freeze operation.

`omega run --both` compares termination only. To check actual Terminal output,
`validation/observe.rs` consumes the same public package/native-product and
Terminal runtime APIs as the shipped CLI. `build_observer.py` builds that
observation-only Rust consumer in an ignored private Cargo project, reusing the
checkout's lock versions and deliberate development hot-path profiles. It compiles once, retains that exact
Terminal artifact and provisions, runs all finite inputs sequentially in fresh
private domains, exports both streams/statuses, publishes the native product,
and runs the same inputs sequentially through that executable. It never
implements scheduling, fabricates providers or executes checked trees.

```text
python3 samples/cli/independent/resource_scheduler/validation/build_observer.py /tmp/ind02-observer-target
/tmp/ind02-observer-target/debug/ind02-observe samples/cli/independent/resource_scheduler/main.omg samples/cli/independent/resource_scheduler/data samples/cli/independent/resource_scheduler/build/observations
python3 samples/cli/independent/resource_scheduler/validation/validate.py samples/cli/independent/resource_scheduler/build/observations
$OMEGA --check --target linux_x86_64 samples/cli/independent/resource_scheduler/intended_invalid/invalid.omg
```

The last command must reject the complete intended-invalid program: its constant
21 exceeds the required `0..=20` start contract. Runtime input rejection is
verified separately and cannot substitute for this source rejection. Observation
output includes native/Terminal `.stdout`, `.stderr`, `.exit`, a retained native
product, its path, and `validation.json`. Build products and acceptance state
are local, ignored artifacts. Run one Rust build and one Omega execution at a
time; this validation consumer also executes every input sequentially. Any
custody failure caused by a synthetic sandbox root must be retried through the
approved per-command execution context, without changing ownership or guards.

## Review checkpoint

See `validation/STATUS.md` for the recorded baseline and validation results.
All 53 native/Terminal cases pass the frozen independent oracle, and the distinct
complete intended-invalid source rejects for its declared range. The bounded
compiler repair preserves reused integer inputs across fixed-register divisions.
The review bundle contains exact commands, source/binary hashes and observed
artifacts. Independent review found no blocking correctness findings. The final
handoff records the integrated validation and main-only publication status.

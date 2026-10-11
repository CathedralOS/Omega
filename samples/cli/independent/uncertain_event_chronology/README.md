# Uncertain event chronology (NEW068)

Reconcile three synthetic quantized event timestamps A, B, C, each on the
integer clock ticks 0 through 9. Bounds are inclusive; a precedence record
(source, destination, lag) means destination >= source + lag. This deliberately
finite scientific sample model makes no inference about continuous time,
measurement distributions, real people, or actual historical events.

The complete Omega application reads one bounded stdin record, validates it,
enumerates all 1000 assignments, and emits the exact count and marginal feasible
ranges. Correlations remain represented by the constraints: the reported ranges
do not promise that every combination of their members is feasible.

For an inconsistent model, a deletion pass removes each edge that is unnecessary
for inconsistency. The four-bit witness identifies an irreducible set of original
precedence records, together with all original interval bounds. It is a finite
contradiction certificate, not a claim of a minimum-cardinality subset or a
printed graph-cycle path. Removing any retained edge admits an assignment.
Positive-lag cycles are negative cycles in the equivalent difference-constraint
graph, and are infeasible. Zero-lag cycles can be feasible.

## Input and output

One record has 19 ASCII decimal digits, optionally followed by LF:

- Six digits: A lower/upper, B lower/upper, C lower/upper.
- One digit: number of active precedence records, 0 through 4.
- Four triples: source event (0 through 2), destination event (0 through 2), lag.
  Unused triples must still contain decimal digits, but have no semantic effect.

For example `0426592012121000000` means bounds [0,4], [2,6], [5,9],
and A+2 <= B, B+1 <= C. It produces 61 feasible assignments and the original
three ranges. Input is read into 32 bytes. Empty, short, overlong, CRLF, nondecimal,
reversed intervals, excessive counts and active invalid endpoints return
`invalid input` and exit 2. Feasible and infeasible models both exit 0.
Trailing input after the first completed line is outside this one-record protocol.

`main.omg` owns input, dispatch and reporting. `chronology/input.omg` validates the
wire record; `model.omg` defines private storage; `solver.omg` owns enumeration
and certificate extraction. The application owns the input backing across the
blocking read; private `Digits` and `Candidate` records own parser and search
scratch storage across state transitions. Only the current bundled std Console/provider is a
dependency. There are no shared numerics imports or active-worker dependencies.

Existing samples were compared before implementation: `maze_flood` performs
single-source maze BFS, `longest_run` scans text, and the dungeon crawler's event
modules dispatch gameplay. None reconciles interval timestamps, quantifies the
entire feasible set, or extracts irreducible inconsistent constraint sets.

## Frozen validation

`data/cases.json` describes 42 input cases: 12 named valid models, 20 seeded
nontrivial variants, and 10 invalid records. Each folder contains stdin, exact
stdout and exit oracles. `validation/reference.py` independently enumerates the
Cartesian product of interval members and verifies every contradiction witness's
inconsistency and irreducibility. All arithmetic is integer: tolerance is zero.
The oracle was frozen before the first Omega check; its SHA-256 identity is in
`validation/oracle-freeze.sha256`. Validation never regenerates expectations. `verify_frozen.py` checks exact raw
input/stdout/exit bytes and both frozen ledgers while retaining the original
reference source unchanged. Two separately frozen supplemental cases exercise
EOF without LF and the unconstrained 1000-assignment upper limit (44 total).

From the repository root, using the compiler built from the recorded base:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
python3 samples/cli/independent/uncertain_event_chronology/validation/verify_frozen.py
sha256sum -c samples/cli/independent/uncertain_event_chronology/validation/oracle-freeze.sha256
python3 samples/cli/independent/uncertain_event_chronology/validation/validate.py --omega target/debug/omega --route check
# Review packages through the documented update/review/resume workflow first.
target/debug/omega update --project samples/cli/independent/uncertain_event_chronology --target linux_x86_64
target/debug/omega --target linux_x86_64 samples/cli/independent/uncertain_event_chronology/main.omg
# Use the executable path printed by successful compilation:
python3 samples/cli/independent/uncertain_event_chronology/validation/validate.py --omega target/debug/omega --route native --executable <published-path>
python3 samples/cli/independent/uncertain_event_chronology/validation/validate.py --omega target/debug/omega --route terminal
python3 samples/cli/independent/uncertain_event_chronology/validation/validate.py --omega target/debug/omega --route invalid
```

Exactly one Rust build and one Omega invocation run at a time. Linux x86-64 is
the declared target; no other target is claimed. Terminal validation explicitly
rejects `DECLINED` as agreement. Reports and observed stdout/stderr go under
ignored `build/validation/`; final evidence records actual supported/blocked
routes, compiler/base identities and commands.

The separate `controls/intended_invalid/main.omg` is a complete source program
that indexes event 3 in a three-element array. Its authored intended rejection
is the documented constant-index upper-bound rule, with the specific expected
fragment in `expected.txt`. Infrastructure/package errors cannot satisfy it.

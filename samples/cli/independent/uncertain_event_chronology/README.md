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

From the repository root on Linux x86-64, using the compiler at the recorded
qualification revision and this origin's accepted package graph:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
APP=samples/cli/independent/uncertain_event_chronology
CLI=target/debug/omega
python3 -B "$APP/validation/verify_frozen.py"
sha256sum -c "$APP/validation/oracle-freeze.sha256"
python3 -B "$APP/validation/validate.py" --omega "$CLI" --route check
"$CLI" --target linux_x86_64 "$APP/main.omg"
# Use the executable path printed by successful compilation:
python3 -B "$APP/validation/validate.py" --omega "$CLI" --route native --executable <published-path>
python3 -B "$APP/validation/validate.py" --omega "$CLI" --route invalid
```

Package preparation must already be accepted for this exact origin. An ordinary
`omega update --project "$APP" --target linux_x86_64` can refresh source/compiler
identity only when all accepted meanings are unchanged and there are zero new
decision rows. Stop on pending authority or denial. Archived locks are evidence;
do not transplant them, repeat the original acceptance, reconstruct grants, or replace a held
package operation with a direct compiler route.
Another consumer path or executor must complete its own normal package review
and explicit admission before running either route. The original accepted record
and this correction do not authorize that consumer; no new admission is performed
by these validation scripts.

For CLI host execution and termination comparison, omit `--target`:

```sh
"$CLI" run --both --keep "$APP/main.omg" < "$APP/data/compatible/input.txt"
python3 -B "$APP/validation/validate.py" --omega "$CLI" --route host-comparison
```

An explicit target makes `run` compile-only, even with `--both`. The corrected
host command replays finite stdin independently to native and Terminal execution,
but CLI stdout/stderr contain the native program streams. `interp exit: N`
reports Terminal termination agreement only. The host-comparison verifier checks
native stdout against the oracle and requires that positive Terminal exit report;
`DECLINED`, missing agreement, divergence and interpretation failures cannot pass.
It does not claim to capture or compare Terminal stdout.

Exact direct Terminal streams use the supported public SDK route below. Build
one Omega CLI, retain its successful Cargo JSON, and link the private sponsor to
that build's public artifacts (`mbx` is the repository's documented Cargo wrapper;
use Cargo when it is unavailable):

```sh
mkdir -p "$APP/build/validation"
mbx build -p omega --message-format=json > "$APP/build/validation/omega-build.jsonl"
python3 -B "$APP/validation/link_terminal.py" "$APP/build/validation/omega-build.jsonl" "$PWD" "$APP/build/validation/terminal-sdk"
python3 -B "$APP/validation/validate.py" --route terminal --terminal-sponsor "$APP/build/validation/terminal-sdk"
```

`terminal_sdk.rs` is the unchanged sponsor source used for the qualified 44-case
SDK observations. It prepares this accepted origin through the package manager,
compiles once, and consumes the retained native product's Terminal artifact and
exact runtime provisions. Each frozen case receives a fresh finite input stream,
host clock snapshot, isolated domain and ordinary 10,000,000 fuel budget.
`provider_proposal` remains `None`; no checked-tree fallback or reconstructed
permission exists. The Terminal verifier uses a fresh output directory, requires
all 44 ordered captures, and checks exact Terminal stdout, empty stderr and exits.
The link receipt records source, CLI, dependency hashes and the exact rustc command.

The actual qualified producer remains
`c8e86802f4cf9fe814d04ece273a9fb2062ba0f1`. Later integrated callback and ordinary-byte
reuse was independently reviewed; it is not a final-main execution claim. The
original 44 native and 44 direct SDK observations and their exact provenance are
preserved in the original version 5 Library packet. On resuming reproduction,
the original accepted-origin lock had been removed with the landed worktree.
The corrected host command was attempted with the qualified CLI and frozen
compatible input, and stopped at missing accepted ProgramEntry authority (exit
200). This is a reproduction blocker, not Terminal runtime success. No evidence
lock was restored and no acceptance was repeated.
Read-only diagnosis confirmed the recreated path and source context match the
original origin. Its `omega.lock` and `build/package-manager/pending` are absent.
Ordinary transaction recovery can complete an existing pending journal, which is
unavailable here; cached historical source bytes supply no accepted policy. The
verified original packet preserves this origin's lock and all 285 unchanged
decision meanings as evidence, without making them replacement authority.

Exactly one Rust build and one Omega invocation run at a time. Linux x86-64 is
the declared target; no other target is claimed. Reports and observed streams go
under ignored `build/validation/`; evidence records supported and blocked routes,
compiler/base identities and commands.

The separate `controls/intended_invalid/main.omg` is a complete source program
that indexes event 3 in a three-element array. Its authored intended rejection
is the documented constant-index upper-bound rule, with the specific expected
fragment in `expected.txt`. Infrastructure/package errors cannot satisfy it.

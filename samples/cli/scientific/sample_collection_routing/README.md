# Sample collection routing (NEW099)

A complete bounded mobile sample-collection planner in Omega. Inputs and the
scientific model are synthetic: two unit-volume vials on a one-dimensional road,
a laboratory at position zero, integer distance units equal to travel minutes,
and one minute of service per pickup/drop. Pickup windows constrain service
start; the depot horizon includes the final drop's service. This is a small
exact planning demonstration, not a clinical scheduling recommendation.

The private search enumerates all 256 four-event words and admits exactly the
permutations obeying pickup/drop precedence, capacity, windows, and depot
completion. The objective minimizes travel distance, then completion time.
All ties on that objective are counted; the lexicographically smallest event
sequence is emitted. Returning to the laboratory for each drop is explicit;
capacity one forces separate collection legs. Capacity two permits combined
collection. The output includes start/end depot, service starts, waiting,
peak and final load, final position, feasible tours and optimal ties.

## Input and output

Provide one eleven-digit ASCII record, optionally followed by LF:
`C A B RA RB LA(2) LB(2) H(2)` with no spaces. Capacity C is 1 or 2; pickup
positions A/B are 1..9; ready times RA/RB are 0..9; latest pickup times LA/LB
and depot horizon H are 00..99. Ready must be no later than latest. Invalid
syntax or bounds exit 64; a valid infeasible model exits 2; an optimum exits 0.
The bounded reader consumes the first record; it accepts EOF after eleven
digits, preserves LF, and rejects CRLF, short lines and overlong first lines.

For `23700999999`, capacity two and positions 3/7 produce distance 14,
completion 18, 6 feasible tours and 4 objective ties. The chosen route is
`0,1,2,3,4,0`: depot, pickup A, pickup B, drop A, drop B, depot. Service
starts are 3,8,16,17. Capacity one instead selects `0,1,3,2,4,0`, distance
20 and completion 24. Every numeric output field has three decimal digits;
[data/frozen_oracles.json](data/frozen_oracles.json) records exact expected bytes.

## Structure and independence

`main.omg` owns input and the entrypoint; `build.omg` binds exact target entry
slots. `input/parse.omg`, `routing/{model,visit,search}.omg`, and
`reporting/certificate.omg` own the whole workflow. The only dependency is the
stable repository std package and existing Console providers. There are no
numerics-library or worker-candidate imports and no shared exports.

Before implementation, existing samples were searched for route, tour, pickup,
capacity and precedence semantics. `algorithms/bubble_sort` supplies array
mutation examples; `collections/inventory_lookup` supplies lookup examples;
`basics/number_guess` is scripted binary search. None optimizes feasible
pickup/drop tours or emits route/objective certificates. This application
adds exhaustive constrained planning and runtime input, rather than renamed
constants or a constant-expression witness.

## Frozen validation

The independent Python oracle enumerates 24 permutations using sets and ordinary
integer arithmetic, while Omega enumerates 256 event words using a private state
machine and flags. The oracle and 25 fixtures were frozen before any Omega run.
Absolute/relative tolerances are both zero. Inputs cover capacity variants,
precedence-sensitive urgent pickup windows, waiting, tied optima, coincident
sites, reversed sites, exact/infeasible deadlines, EOF, and malformed records.
`validate.py` checks fixture hashes, exact stdout and exit codes and writes
actual stdout/stderr under ignored `build/validation-{native,both}`. It never
accepts packages or regenerates expected data. `--both` also requests Terminal
comparison; a DECLINED comparison remains an explicit unsupported result.

The separate [intended-invalid source](controls/intended_invalid/main.omg)
is a complete dependency-free package source that writes index four of a four-slot carrier.
The documented rule is [collection bounds](../../../../wiki/spec/language/numeric_values.md#collection-bounds).
Its expected rejection is `cannot prove index \`4\` is within length 4`.
A package-review, parser, or other rejection is not an invalid-control pass.

## Reproduce through ordinary facilities

From the repository root, set `OMEGA_LIBRARY_ROOT` to that checkout's
`source/library`, and use the compiler binary built from that revision. Build
one Rust compiler and execute one Omega command at a time. Substitute the
verified absolute binary path for `$omega_binary` below.

```sh
sample=samples/cli/independent/sample_collection_routing
"$omega_binary" update --project "$sample" --target linux_x86_64
# Inspect the generated review and its findings; edit each pending decision.
# Resume only an accepted review, and never retry a denied acceptance.
"$omega_binary" update --resume --project "$sample"
"$omega_binary" --check --target linux_x86_64 --build-dir "$sample/build/check" "$sample/main.omg"
python3 "$sample/validate.py" --omega "$omega_binary" --route native
# Or compile once and pass the exact artifact path from the compiler report:
# python3 "$sample/validate.py" --executable "$reported_artifact" --route native
python3 "$sample/validate.py" --omega "$omega_binary" --route both
"$omega_binary" --check --target linux_x86_64 --build-dir "$sample/build/invalid" "$sample/controls/intended_invalid/main.omg"
```

The control has no external package review; a lock from another
worktree is not acceptance for the valid application. macOS ARM64 and Windows x86-64 have declared entry
bindings for the valid app; their execution is not implied by Linux evidence.
The delivered review bundle includes `evidence/validation.md` with observed
commands, identities and limitations. These records live outside the project
during validation so that writing them cannot change accepted source identity.
Generated output and local lock acceptance are excluded from the candidate;
review on another executor must establish its own package acceptance.

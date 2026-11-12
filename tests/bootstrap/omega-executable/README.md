# Omega source to an Alpha executable

This gate connects the manifested Epsilon-written bootstrap compiler to
its existing parser and Alpha tape encoder through the real request route:
the gate frames the canonical OCREQ V1 request (one application package
snapshotting [program.omg](program.omg) as a single `.omg` regular file,
empty edges, `alpha_bootstrap_tape` product on the `alpha_bootstrap` profile,
bound SHA-256 subject commitment), and
[main_ocreq.epsilon](main_ocreq.epsilon) runs the bootstrap compiler's
`OmegaRequestStructure::check` shape passes, extracts the root package's single
`.omg` member, compiles it, and publishes the unwrapped artifact on `Complete`
or exactly one OCOUT V1 frame on failure. The admitted slice is
`alpha_bootstrap_tape` plus a single-source application root; shape-valid
requests outside it refuse `malformed_request` at the first out-of-slice
coordinate. The target's ProgramEntry contract names the entry machine `main`,
so entry selection is bound by the route, not by an adapter-supplied spelling.
The emitted tape halts Alpha with the selected machine's return value. The
expected result is [42](expected.txt).

`--diagnostic` keeps the retired raw-source adapter
([main.epsilon](main.epsilon)) reachable for refusal coverage:
`python3 gate.py "$OUTPUT_DIR" "$OMEGA_PATH_EPSILON_EXECUTION_DRIVER" --diagnostic`.

From the repository root on macOS arm64, Linux x86-64, or Windows x64 with Git Bash:

```sh
sh tests/bootstrap/omega-executable/run.sh
```

The gate requires Python 3, the selected checked-in Alpha seed and the existing
shell tools; macOS also requires `codesign`. It stamps the separately admitted
[Beta tape](../../../bootstrap/1_beta/AUDIT.md) into the selected Alpha container,
rebuilds Beta from source, and uses the rebuilt compiler to
construct the Gamma evaluator from source, and compares both tapes exactly
with their bound artifacts. It executes those constructed bytes to reconstruct
the Epsilon execution receipt through the Gamma-written Delta compiler, checks
the Epsilon-written bootstrap compiler plus its request-route entry,
seals the source into the canonical request, and stamps and executes only the
actual successful output.
No Rust compiler, host parser, host typechecker, or host code generator supplies
the program's meaning. The packing and stamping helpers remain trusted byte
transport. Python frames, invokes, compares, and stores bytes. The separate
[Alpha-to-Beta gate](../alpha-beta-edge.sh) also checks native-container source
provenance; this ordinary executable gate does not rebuild the native seed.

Outputs live in ignored `build/omega-executable/`, including `program.tape` and
`program.exe`. The executable is the host's Alpha VM container carrying the
emitted tape, not direct native lowering of the Omega body. The evaluator and
packed implementation files remain there for inspection. A failed compilation
does not replace the last successful tape; prior files are not evidence of a
new successful run. The command's successful final result is the acceptance gate.

The evaluator reconstruction has a 1,800-second watchdog, compiler execution has
14,400 seconds by default, and the emitted program has 30 seconds. Expiration
kills the invoked process tree and is not an Omega result. Set
`OMEGA_EXECUTABLE_RECEIPT_SECONDS` to a positive number for another
reconstruction allowance and `OMEGA_EXECUTABLE_OBSERVATION_SECONDS` for another
compiler observation allowance; both receipts stay pinned to the selected gate
identity regardless of the allowance. `OMEGA_EXECUTABLE_BUILD_DIR` selects a
different output directory. These are host controls, not language semantics.

## Current compiler slice

[The bootstrap compiler's coverage and storage contract](../../../bootstrap/5_omega/README.md)
is authoritative for this gate. It currently admits a nullary `u8` entry,
plain `u8` parameters/results, immutable initialized locals, resolved free
calls, nested calls as complete arguments, and constant-selected state
transfers. It checks all bodies and rejects reentrant ordinary-call graphs
before emission. The private static cells preserve caller values and pending
arguments. Unsupported nested ordinary calls cannot fall through to globals.

Anonymous constant arithmetic is a bounded exact subset: integral `+ - * /`
with every intermediate in `0..255`, comparisons, and Boolean operations.
Fractional, signed or wider intermediates, anonymous remainder, and
width-dependent anonymous operations refuse as unsupported. A destination
annotation never changes anonymous division into truncating integer division.
Runtime arithmetic, richer carriers, clauses, mutable data and dynamic control
remain outside current emission coverage.

The route selects `main` — the machine name `alpha_bootstrap::ProgramEntry`
binds. Another source can be exercised without changing compiler code:

```sh
sh tests/bootstrap/omega-executable/run.sh --source path/to/program.omg --expect 7
```

`--expect-compile 1` expects a compiler rejection; `--expect-compile 2` expects
an unsupported form. Both require an exact failure outcome: the request route
publishes one closed OCOUT frame, the diagnostic lane publishes no tape.
The harness owns those diagnostic statuses; they are not new OCOUT wire numbers.
Resource and internal failures remain distinct from successful compilation.

Every invocation first reconstructs the same pinned Epsilon execution receipt —
identical request, identical 721,484-byte output — so
`OMEGA_EXECUTABLE_RECEIPT_CACHE=<file>` lets the controls matrix share one
reconstruction; cached bytes still face the pinned identity check, and
`OMEGA_EXECUTABLE_RECEIPT_SECONDS` bounds a cold reconstruction.

## Regression controls

```sh
sh tests/bootstrap/omega-executable/run.sh --controls
sh tests/bootstrap/omega-executable/run.sh --controls-b
sh tests/bootstrap/omega-executable/run.sh --controls-c
sh tests/bootstrap/omega-executable/run.sh --controls-d
sh tests/bootstrap/omega-executable/run.sh --controls-e
sh tests/bootstrap/omega-executable/run.sh --controls-f
sh tests/bootstrap/omega-executable/run.sh --controls-g
sh tests/bootstrap/omega-executable/run.sh --controls-h
sh tests/bootstrap/omega-executable/run.sh --controls-i
sh tests/bootstrap/omega-executable/run.sh --controls-j
```

The ordinary Epsilon controls reuse one compiler across eighty-three
invocations split among [controls.epsilon](controls.epsilon),
[controls_b.epsilon](controls_b.epsilon),
[controls_c.epsilon](controls_c.epsilon),
[controls_d.epsilon](controls_d.epsilon),
[controls_e.epsilon](controls_e.epsilon),
[controls_f.epsilon](controls_f.epsilon),
[controls_g.epsilon](controls_g.epsilon),
[controls_h.epsilon](controls_h.epsilon),
[controls_i.epsilon](controls_i.epsilon), and
[controls_j.epsilon](controls_j.epsilon): zero and maximum byte results,
folded integral arithmetic and explicit anonymous bitwise/shift refusals,
multiple declarations and selection of a non-first entry, duplicate names,
missing entry, out-of-range and oversized decimal values, an out-of-range
expression operand, overflow, underflow, division and modulo by zero,
out-of-width and overflowing shifts, an invalid unselected body, unsupported
path, `-` unary, and other expressions and return types, a
named-state-only machine, digit separators, malformed syntax, and successful
reuse after failures; then checked control flow: call statements and grouped
terminal calls between machines with exact emitted-tape assertions,
subjectless and literal-subject transitions into authored states, folded
Boolean subjects through the comparison, logical, and `!` operators, a
folded Boolean refused at the `u8` terminal, carrier-class rejections at
operators (`&&` on u8 operands, `<` against a Boolean, `!` on a u8, `==`
across classes), a forward-declared callee, machine-scoped state
resolution, unresolved call
and state targets, call and transition arity mismatches, an empty authored
state, an unmatched subject, duplicate authored state names, an unreachable
statement after a transition block, receiver and `self` call paths, a
parameterized state, a multi-arm or non-wildcard subjectless block, an
integer arm under a Boolean subject, refused fractional anonymous division,
unsupported shift and bitwise expressions, an ungrouped terminal call, two sequential call
statements, a wildcard arm ahead of a matching literal arm, and a Boolean arm
under an integer subject.
Every rejection requires an empty unsealed
tape. A final exact literal byte comparison precedes execution of the actual
emitted tape; the expected bytes are never used as the executable input.

The split is an evaluator-run boundary, not a compiler one: each invocation
is a fresh evaluation. It predates the V5 pair-arena growth — the retired
40,265,318-node arena, refused with status 252, could not retain even an
eighteen-invocation half of the matrix — and remains the bounded evaluated
form. Each part carries a bounded run of controls and ends with the same
`20 + 22` success compile, byte `67`, and tape publication, so every run
checks the same observation and executes the same emitted program.

## Bound customer entries

Every entry the harness may select is bound: `main_ocreq.epsilon`,
`main.epsilon`, and the ten controls files are gate-local inputs packed on
top of the bound member closure, never part of the manifested members.

| Entry | Bytes | SHA-256 |
| --- | ---: | --- |
| `main.epsilon` | 1,757 | `c0af3126f13c8c511d04f224e630f60f3316c0f9fa6e310e72f66779c7c3ce9e` |
| `main_ocreq.epsilon` | 19,249 | `5d5d0b8ed0146b055ffdbb6d680bb902a0e350c80b13577bf148879c6c753943` |
| `controls.epsilon` | 3,536 | `f30a9a3206fdbe7a13b1b6377af7b5d95bca5e27c495bf4faab262fcc0525089` |
| `controls_b.epsilon` | 2,804 | `28701862d665951b2e432c58c77b7b1bcbc9d2b8ae53417f95bc76fa19908c6e` |
| `controls_c.epsilon` | 2,834 | `1ec2b16b0d97631bc88f23c0d39c417e6184875c1178913fba4629b815fc3465` |
| `controls_d.epsilon` | 2,852 | `b54adbcd30b9f192217b89b96efd3b548443a6e9426bb7891c98845fa89e8adb` |
| `controls_e.epsilon` | 2,797 | `83ce536dacd5efb9238d7f5869ed5d6269c6481a24b4cdd0b7d3985777c150bc` |
| `controls_f.epsilon` | 4,425 | `fbc7ed2868f9e70833fdfc36c927238c8fd11184e5127e372d732c8ab6ebff0e` |
| `controls_g.epsilon` | 3,127 | `3c94d2e5430226dbeb20b311d5336f57ab11c8fd49ace137e44785fcbac6ecb9` |
| `controls_h.epsilon` | 3,193 | `b48c672f09c8263d9d352fdb37af66a82c3083df38dabd533a93e0573e9e5c0e` |
| `controls_i.epsilon` | 3,722 | `9c7d77740284bbca570197290d2d73188681b3763e15a22bf3ce5592f4c12846` |
| `controls_j.epsilon` | 4,461 | `406e2bc4353983ebc81d6daa215a9b42f72b85056bc5f3263e1917e7e01efb94` |

`tools/bootstrap/omega/compiler_env.sh` checks every entry identity before
each packing and `tests/bootstrap/omega-identity.sh` covers the refusals. The
customer is the bound compiler closure followed by the selected entry. Each
execution reports its concatenated size and digest; those observations do not
introduce a second table of source identities.

## Execution boundary

This is a component-level source-to-executable regression, not the final
[standalone compiler interface](../../../wiki/spec/build/compiler_request.md).
It uses the existing private Epsilon execution observation format. It does not
implement package resolution, Build evaluation, general ProgramEntry
selection beyond the target's bound `main` name, general Omega checking, or a
compiler-refinement proof. `main_ocreq.epsilon` is the gate's entry: the real
OCREQ/OCOUT route for the `alpha_bootstrap_tape` slice, bound like every
other selectable entry. `main.epsilon` remains bound for the `--diagnostic`
raw-source lane.

The additional [controls_values.epsilon](controls_values.epsilon) customer uses
an internal selected-entry boundary to check parameter/local/call lowering and
negative cases, then publishes length-framed artifacts for actual Alpha
execution. `python3 bootstrap/compiled_vm/source_test.py --omega-values`
constructs its source-only semantic producer path. Each invocation stores the
exact packed compiler, complete customer, request and observation; historical
measurements remain bound to their original sources. This customer is separate
from the public OCREQ admission checks.

# Omega source to an Alpha executable

This gate connects the complete manifested Epsilon-written Omega compiler to
its existing parser and Alpha tape encoder through the real request route:
the gate frames the canonical OCREQ V1 request (one application package
snapshotting [program.omg](program.omg) as a single `.omg` regular file,
empty edges, `alpha_bootstrap_tape` product on the `alpha_bootstrap` profile,
bound SHA-256 subject commitment), and
[main_ocreq.epsilon](main_ocreq.epsilon) runs D's own
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

From the repository root on macOS arm64, or Windows x64 with Git Bash:

```sh
sh tests/bootstrap/omega-executable/run.sh
```

The gate requires Python 3, the selected checked-in Alpha seed and the existing
shell tools; macOS also requires `codesign`. It reconstructs the Epsilon
execution receipt through the selected Gamma-written Delta compiler, checks
the complete Epsilon-written Omega compiler plus its request-route entry,
seals the source into the canonical request, and stamps and executes only the
actual successful output.
No Rust compiler, host parser, host typechecker, or host code generator supplies
the program's meaning. Python frames, invokes, compares, and stores bytes.

Outputs live in ignored `build/omega-executable/`, including `program.tape` and
`program.exe`. The executable is the host's Alpha VM container carrying the
emitted tape, not direct native lowering of the Omega body. The evaluator and
packed implementation files remain there for inspection. A failed compilation
does not replace the last successful tape; prior files are not evidence of a
new successful run. The command's successful final result is the acceptance gate.

The evaluator reconstruction has a 300-second watchdog, compiler execution has
14,400 seconds by default, and the emitted program has 30 seconds. Expiration
kills the invoked process tree and is not an Omega result. Set
`OMEGA_EXECUTABLE_RECEIPT_SECONDS` to a positive number for another
reconstruction allowance and `OMEGA_EXECUTABLE_OBSERVATION_SECONDS` for another
compiler observation allowance; both receipts stay pinned to the selected gate
identity regardless of the allowance. `OMEGA_EXECUTABLE_BUILD_DIR` selects a
different output directory. These are host controls, not language semantics.

## Current compiler slice

[Scalar compilation](../../../bootstrap/5_omega/scalar_compilation.epsilon)
accepts ordinary nullary free machines returning a `u8` terminal expression:
unsuffixed nonnegative decimal literals without digit separators joined by the
`+ - * / % << >> & | ^` binary operators and the `~` bitwise-complement
prefix, folded under the default Exact policy so every intermediate node
must stay representable in `u8`. It checks every
admitted declaration, detects duplicate names,
resolves an explicitly supplied entry name, checks the result range before
emission, and uses the shared encoder's labels, call/return and finalization.
Multiple admitted machines are emitted in authored order; neither the source
filename nor the value 42 has special meaning to the compiler.

The same slice admits bounded control flow. A state body is zero or more
nullary call statements to free machines — the machine-call edge carries a
return address and leaves the callee's `r0` — followed by exactly one
terminal: a folded `u8` expression returning in `r0`, a grouped nullary call
expression such as `(helper())` that returns the callee's `r0`, or a
`transition` block. A transition targets authored states of the same
machine; a subjectless block admits exactly one wildcard arm, and a
subjectful block folds its subject and selects the first matching integer,
Boolean, or wildcard arm. Every arm's target resolves and matches the
target's parameter count before any emission, so a transition is a checked
direct jump — no call frame, no return edge — exactly as the state contract
specifies. Receivers, `self` paths, machine or runtime arguments,
parameterized states, locals, assignments, `let` bindings, and non-literal
guards stay `Incomplete` coverage refusals.

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
```

The ordinary Epsilon controls reuse one compiler across sixty-six
invocations split among [controls.epsilon](controls.epsilon),
[controls_b.epsilon](controls_b.epsilon),
[controls_c.epsilon](controls_c.epsilon),
[controls_d.epsilon](controls_d.epsilon),
[controls_e.epsilon](controls_e.epsilon),
[controls_f.epsilon](controls_f.epsilon),
[controls_g.epsilon](controls_g.epsilon), and
[controls_h.epsilon](controls_h.epsilon): zero and maximum byte results,
folded arithmetic, bitwise and shift operations with precedence and grouping,
multiple declarations and selection of a non-first entry, duplicate names,
missing entry, out-of-range and oversized decimal values, an out-of-range
expression operand, overflow, underflow, division and modulo by zero,
out-of-width and overflowing shifts, an invalid unselected body, unsupported
comparison, path, `!`/`-` unary, and other expressions and return types, a
named-state-only machine, digit separators, malformed syntax, and successful
reuse after failures; then checked control flow: call statements and grouped
terminal calls between machines with exact emitted-tape assertions,
subjectless and literal-subject transitions into authored states, a
forward-declared callee, machine-scoped state resolution, unresolved call
and state targets, call and transition arity mismatches, an empty authored
state, an unmatched subject, duplicate authored state names, an unreachable
statement after a transition block, receiver and `self` call paths, a
parameterized state, a multi-arm or non-wildcard subjectless block, and an
integer arm under a Boolean subject.
Every rejection requires an empty unsealed
tape. A final exact literal byte comparison precedes execution of the actual
emitted tape; the expected bytes are never used as the executable input.

The split is an evaluator-run boundary, not a compiler one: each invocation
is a fresh evaluation. It predates the V5 pair-arena growth — the retired
40,265,318-node arena, refused with status 252, could not retain even an
eighteen-invocation half of the matrix — and remains the bounded evaluated
form. Each part carries seven or fewer controls and ends with the same
`20 + 22` success compile, byte `67`, and tape publication, so every run
checks the same observation and executes the same emitted program.

## Bound customer entries

Every entry the harness may select is bound: `main_ocreq.epsilon`,
`main.epsilon`, and the eight controls files are gate-local inputs packed on
top of the bound member closure, never part of the manifested members.

| Entry | Bytes | SHA-256 | Packed customer bytes | Packed customer SHA-256 |
| --- | ---: | --- | ---: | --- |
| `main.epsilon` | 1,757 | `c0af3126f13c8c511d04f224e630f60f3316c0f9fa6e310e72f66779c7c3ce9e` | 560,910 | `bc7b11ffb321aaab3a2a81b19c89288c0c6b30a3e5432e19efc44caa75927992` |
| `main_ocreq.epsilon` | 19,253 | `9573d73423c2ed3e586b0298ae333733d8828c958a38f1859e1baff3d5ac4a9d` | 578,406 | `f1ba2ea384e4f6f264d4cfe15052fc3d8412a981f9eee1542b46596714a6dae7` |
| `controls.epsilon` | 3,631 | `78995d1f7975bbd7b8d82230b557f43bb263addb5be3deb2b9bf56cb03efa0a9` | 562,784 | `f7598fc60621df7d115d61d7fadb23cdbbafa5e41a176b15fc1ffeff3c5f1914` |
| `controls_b.epsilon` | 3,084 | `261d1529b50ab7b36c9dd228a0df7a4250d46d2913dcd85897ee8b911e98dbc3` | 562,237 | `20ab420819bda1cc2b3bee2feb95b44a057274f8b724723a25eac7318e2ef4f1` |
| `controls_c.epsilon` | 2,824 | `0dbc7da705e7da63a7589b49a25677037dcd43c31c3266f31511986b3eba54ae` | 561,977 | `7b97fad5b22f1b6ac38673946c63892315ef46e101867e5f8e19f885399e67ed` |
| `controls_d.epsilon` | 2,850 | `916218b57476fe59f22a2d493b6529502e3ac3a4fda856d4e16b9a156a0f57c9` | 562,003 | `c8d2cdbe21d472b81327d383b6850738d6cd920f605ca3fdebe8b2b1a18fa488` |
| `controls_e.epsilon` | 2,797 | `83ce536dacd5efb9238d7f5869ed5d6269c6481a24b4cdd0b7d3985777c150bc` | 561,950 | `16b20dbbb30647f41f749d341a5d0875ce0048d7c6d270ff1862747b0aab77d9` |
| `controls_f.epsilon` | 4,425 | `fbc7ed2868f9e70833fdfc36c927238c8fd11184e5127e372d732c8ab6ebff0e` | 563,578 | `e3b53686807708356950b719205994348be223110563c40508db7087c3eaec3a` |
| `controls_g.epsilon` | 3,127 | `3c94d2e5430226dbeb20b311d5336f57ab11c8fd49ace137e44785fcbac6ecb9` | 562,280 | `199ceaae8285898d7cbd38deff6ec0e761e0d2b8a0d7329e7ffdeb32c9c29dea` |
| `controls_h.epsilon` | 3,193 | `b48c672f09c8263d9d352fdb37af66a82c3083df38dabd533a93e0573e9e5c0e` | 562,346 | `05d9b92950c61a67104b75851838b7fe37dd3f9d841fc894c20d52b7b83d0d5e` |

`tools/bootstrap/omega/compiler_env.sh` checks every entry identity before
each packing and `tests/bootstrap/omega-identity.sh` covers the refusals and
verifies each packed customer, compiler bytes plus entry, against this table.

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
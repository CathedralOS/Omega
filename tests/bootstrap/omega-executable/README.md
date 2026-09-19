# Omega source to an Alpha executable

This gate connects the complete manifested Epsilon-written Omega compiler to
its existing parser and Alpha tape encoder. The input is
[ordinary Omega source](program.omg), not a hand-authored tape. Its selected
machine returns a byte; the diagnostic entry adapter calls that machine and
halts Alpha with its return value. The expected result is [42](expected.txt).

From the repository root on macOS arm64, or Windows x64 with Git Bash:

```sh
sh tests/bootstrap/omega-executable/run.sh
```

The gate requires Python 3, the selected checked-in Alpha seed and the existing
shell tools; macOS also requires `codesign`. It reconstructs the Epsilon
execution receipt through the selected Gamma-written Delta compiler, checks
the complete Epsilon-written Omega compiler plus its diagnostic harness, feeds
it the source bytes, and stamps and executes only the actual successful output.
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
`+ - * / % << >> & | ^` binary operators, folded under the default Exact policy
so every intermediate node must stay representable in `u8`. It checks every
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

The harness selects `answer`. Another source can be exercised without changing
compiler code:

```sh
sh tests/bootstrap/omega-executable/run.sh --source path/to/program.omg --expect 7
```

`--expect-compile 1` expects a compiler rejection; `--expect-compile 2` expects
an unsupported form. Both require an exact failure outcome without a tape prefix.
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

The ordinary Epsilon controls reuse one compiler across sixty-three
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
comparison, path, unary and other expressions and return types, a
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

## Execution boundary

This is a component-level source-to-executable regression, not the final
[standalone compiler interface](../../../wiki/spec/build/compiler_request.md).
It uses the existing private Epsilon execution observation format. It does not
implement package resolution, Build evaluation, ProgramEntry selection, general
Omega checking, or a compiler-refinement proof. The scalar invocation adapter
is explicit test machinery, not a claim to implement the target's final entry
contract. Retire it when the real sealed request and target entry route cover
the same source, selection and failure controls.
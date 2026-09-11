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
`OMEGA_EXECUTABLE_OBSERVATION_SECONDS` to a positive number for another compiler
observation allowance. `OMEGA_EXECUTABLE_BUILD_DIR` selects a different output
directory. These are host controls, not language semantics.

## Current compiler slice

[Scalar compilation](../../../bootstrap/5_omega/scalar_compilation.epsilon)
accepts ordinary nullary free machines returning unsuffixed nonnegative decimal
`u8` literals without digit separators. It checks every admitted declaration, detects duplicate names,
resolves an explicitly supplied entry name, checks the result range before
emission, and uses the shared encoder's labels, call/return and finalization.
Multiple admitted machines are emitted in authored order; neither the source
filename nor the value 42 has special meaning to the compiler.

The harness selects `answer`. Another source can be exercised without changing
compiler code:

```sh
sh tests/bootstrap/omega-executable/run.sh --source path/to/program.omg --expect 7
```

`--expect-compile 1` expects a compiler rejection; `--expect-compile 2` expects
an unsupported form. Both require an exact failure outcome without a tape prefix.
The harness owns those diagnostic statuses; they are not new OCOUT wire numbers.
Resource and internal failures remain distinct from successful compilation.

## Regression controls

```sh
sh tests/bootstrap/omega-executable/run.sh --controls
```

The [ordinary Epsilon controls](controls.epsilon) reuse one compiler across
thirteen invocations: zero and maximum byte results, multiple declarations and
selection of a non-first entry, duplicate names, missing entry, out-of-range
and oversized decimal values, an invalid unselected body, unsupported expressions
and return types, a named-state-only machine, digit separators, malformed syntax,
and successful reuse after failures. Every rejection requires an empty unsealed
tape. A final exact literal byte comparison precedes execution of the actual
emitted tape; the expected bytes are never used as the executable input.

## Execution boundary

This is a component-level source-to-executable regression, not the final
[standalone compiler interface](../../../wiki/spec/build/compiler_request.md).
It uses the existing private Epsilon execution observation format. It does not
implement package resolution, Build evaluation, ProgramEntry selection, general
Omega checking, or a compiler-refinement proof. The scalar invocation adapter
is explicit test machinery, not a claim to implement the target's final entry
contract. Retire it when the real sealed request and target entry route cover
the same source, selection and failure controls.
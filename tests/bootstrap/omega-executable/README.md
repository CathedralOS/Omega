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
sh tests/bootstrap/omega-executable/run.sh --controls-i
```

The ordinary Epsilon controls reuse one compiler across seventy
invocations split among [controls.epsilon](controls.epsilon),
[controls_b.epsilon](controls_b.epsilon),
[controls_c.epsilon](controls_c.epsilon),
[controls_d.epsilon](controls_d.epsilon),
[controls_e.epsilon](controls_e.epsilon),
[controls_f.epsilon](controls_f.epsilon),
[controls_g.epsilon](controls_g.epsilon),
[controls_h.epsilon](controls_h.epsilon), and
[controls_i.epsilon](controls_i.epsilon): zero and maximum byte results,
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
parameterized state, a multi-arm or non-wildcard subjectless block, an
integer arm under a Boolean subject, truncating division, additive-versus-shift
and bitwise-tier precedence, an ungrouped terminal call, two sequential call
statements, a wildcard arm ahead of a matching literal arm, and a Boolean arm
under an integer subject.
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

Every entry the harness may select is bound: `main.epsilon` and the nine
controls files are gate-local inputs packed on top of the bound member
closure, never part of the manifested members.

| Entry | Bytes | SHA-256 | Packed customer bytes | Packed customer SHA-256 |
| --- | ---: | --- | ---: | --- |
| `main.epsilon` | 1,759 | `4fb023e60c166d5700fddc343a8ee8f3242d3c2915e7a7556ec36bef19aded9b` | 559,920 | `7c9112e3c15d995c8d5d188b2568cb39915992c9769206924463c314a84a3948` |
| `controls.epsilon` | 3,339 | `44b8f0d15df414a80728918560ef988341537cfa25c0e21d6240a52c7f72f91b` | 561,500 | `f22dbae93f2b1cc4478fdaf1d7053dd2d9f6405bb1d651fec13a234e884361b8` |
| `controls_b.epsilon` | 3,084 | `261d1529b50ab7b36c9dd228a0df7a4250d46d2913dcd85897ee8b911e98dbc3` | 561,245 | `e9d4627c6226ba39359708d5163010499210386dcb51e82577e89e757734a5c1` |
| `controls_c.epsilon` | 2,824 | `0dbc7da705e7da63a7589b49a25677037dcd43c31c3266f31511986b3eba54ae` | 560,985 | `72ef281f99dd95a96287807d3e7d200b412aa831e570a3c290474eec59046d53` |
| `controls_d.epsilon` | 2,850 | `916218b57476fe59f22a2d493b6529502e3ac3a4fda856d4e16b9a156a0f57c9` | 561,011 | `69fb11609028859b5584d464f1551269c0d0571aa7470e8e9d9bfec789253a82` |
| `controls_e.epsilon` | 2,703 | `42090d41fbfa2068e1063bebfa7caae4373ac7b5825ede94b8edbc8877338248` | 560,864 | `f6d343ba22a3690ae7d42f47f41d0e65f7890dfe5ad7696626abb100d9288e4a` |
| `controls_f.epsilon` | 4,425 | `fbc7ed2868f9e70833fdfc36c927238c8fd11184e5127e372d732c8ab6ebff0e` | 562,586 | `e3712a9642ade95f7733307198734da2613d186baf9e02b3ee40378e13178821` |
| `controls_g.epsilon` | 3,127 | `3c94d2e5430226dbeb20b311d5336f57ab11c8fd49ace137e44785fcbac6ecb9` | 561,288 | `e9586c29c06358aef499ace1dc3f49144dca86a5bc24707d73b0639ce2f556de` |
| `controls_h.epsilon` | 3,193 | `b48c672f09c8263d9d352fdb37af66a82c3083df38dabd533a93e0573e9e5c0e` | 561,354 | `c461e4ac6b09b109962148618880186d32ec8a87300befe53ae41814ff2cbdc5` |
| `controls_i.epsilon` | 3,863 | `8f583b6510c940e3ef0cdc0223a1e263da37ea4f6decbea1a3130f4ba33645d0` | 562,024 | `c8f075aa5015f9bf7850f7c33001e88b7ceba75afcb62aeb4f8934cd16f9ecd0` |

`tools/bootstrap/omega/compiler_env.sh` checks every entry identity before
each packing and `tests/bootstrap/omega-identity.sh` covers the refusals and
verifies each packed customer, compiler bytes plus entry, against this table.

## Execution boundary

This is a component-level source-to-executable regression, not the final
[standalone compiler interface](../../../wiki/spec/build/compiler_request.md).
It uses the existing private Epsilon execution observation format. It does not
implement package resolution, Build evaluation, ProgramEntry selection, general
Omega checking, or a compiler-refinement proof. The scalar invocation adapter
is explicit test machinery, not a claim to implement the target's final entry
contract. Retire it when the real sealed request and target entry route cover
the same source, selection and failure controls.
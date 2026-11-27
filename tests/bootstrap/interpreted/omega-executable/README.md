# Omega source to an Alpha executable

This gate connects the manifested Epsilon-written bootstrap compiler to
its existing parser and Alpha tape encoder through the real request route:
the gate frames the canonical OCREQ V1 request (one application package
snapshotting [program.omg](../../conformance/omega/program.omg) as a single `.omg` regular file,
empty edges, `alpha_bootstrap_tape` product on the `alpha_bootstrap` profile,
bound SHA-256 subject commitment), and
[main_ocreq.epsilon](../../conformance/omega/main_ocreq.epsilon) runs the bootstrap compiler's
`OmegaRequestStructure::check` shape passes, extracts the root package's single
`.omg` member, compiles it, and publishes the unwrapped artifact on `Complete`
or exactly one OCOUT V1 frame on failure. The admitted slice is
`alpha_bootstrap_tape` plus a single-source application root; shape-valid
requests outside it refuse `malformed_request` at the first out-of-slice
coordinate. The target's ProgramEntry contract names the entry machine `main`,
so entry selection is bound by the route, not by an adapter-supplied spelling.
The emitted tape halts Alpha with the selected machine's return value. The
expected result is [42](../../conformance/omega/expected.txt).

The test-owned [execution driver](execution_driver.delta) frames the Epsilon
source and sealed stdin for the interpreted Omega gate. `--diagnostic` keeps the retired raw-source adapter
([main.epsilon](../../conformance/omega/main.epsilon)) reachable for refusal coverage:
`python3 gate.py "$OUTPUT_DIR" "$OMEGA_PATH_EPSILON_EXECUTION_DRIVER" --diagnostic`.

From the repository root on macOS arm64, Linux x86-64, or Windows x64 with Git Bash:

```sh
sh tests/bootstrap/interpreted/omega-executable/run.sh
```

The gate requires Python 3, the selected checked-in Alpha seed and the existing
shell tools; macOS also requires `codesign`. It stamps the separately admitted
[Beta tape](../../../../bootstrap/beta/AUDIT.md) into the selected Alpha container,
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

[The bootstrap compiler's coverage and storage contract](../../../../bootstrap/omega/compiler/README.md)
is authoritative for this gate. It admits a nullary `u8` entry and checked
`u8`, `u32`, bounded-literal `u64`, `bool`, and `u32 in Trapping` values through parameters, locals,
free calls and same-owner parameterized transfers. Runtime comparisons and
short-circuit Boolean control preserve those types. All bodies and arms are
checked before emission; reentrant ordinary-call graphs and unsupported
facilities refuse without publishing a partial tape. Copy records, zero-established
affine locals, projected field writes, Unit completion and scoped record-reference
parameters now support the actual SourceUnit clear helper. Arithmetic includes proved
unsigned operations, checked Trapping `u32` add/subtract/multiply and `u64`
add/subtract, and Wrapping `u64` add/subtract/multiply. The compiler README specifies the precise storage and loan boundaries.

The source-built compiled peer also exposes `--omega-control`. It takes exact
identifier and numeric digit declarations from the real lexer source and appends
[lexer_controls.omg](../../conformance/omega/lexer_controls.omg), which exercises ASCII interval edges
and high `u32` inputs, every byte against four numeric bases, and digit values
at both ends of the decimal, uppercase and lowercase ranges. [controls_control.epsilon](../../conformance/omega/controls_control.epsilon)
compiles that input plus generic parallel-transfer, comparison, short-circuit,
and refusal/reset controls. The harness executes every emitted artifact through
the Alpha seed. The compiler has no special case for this customer.

The peer's `--omega-storage` customer extracts exact SourceId, SourceUnit,
ModulePath and clear/append/read declarations from the actual source package,
then appends [storage_controls.omg](../../conformance/omega/storage_controls.omg).
[controls_storage.epsilon](../../conformance/omega/controls_storage.epsilon) checks that program plus
11 generic storage programs, 36 explicit refusals and a reset program; the
harness executes all 13 published artifacts. The real helpers retain their
zero/full behavior and byte values while explicitly stating the guarded private
bounds. Companion programs check captured index evaluation, pure guarded entry
requirements, incoming-reference forwarding, and disjoint mutation. Negative
controls include an overlapping mutating call in an earlier argument of the
same expression, stale entry facts, mutating guards and local-backed tail loans.
These internal single-source customers do not claim full package admission.

The route selects `main` — the machine name `alpha_bootstrap::ProgramEntry`
binds. Another source can be exercised without changing compiler code:

```sh
sh tests/bootstrap/interpreted/omega-executable/run.sh --source path/to/program.omg --expect 7
```

`--expect-compile 1` expects a compiler rejection; `--expect-compile 2` expects
an unsupported form. Both require an exact failure outcome: the request route
publishes one closed OCOUT frame, the diagnostic lane publishes no tape.
The harness owns those diagnostic statuses; they are not new OCOUT wire numbers.
Resource and internal failures remain distinct from successful compilation.

Every invocation first reconstructs the same pinned Epsilon execution receipt —
identical request, identical 719,068-byte output — so
`OMEGA_EXECUTABLE_RECEIPT_CACHE=<file>` lets the controls matrix share one
reconstruction; cached bytes still face the pinned identity check, and
`OMEGA_EXECUTABLE_RECEIPT_SECONDS` bounds a cold reconstruction.

## Regression controls

```sh
sh tests/bootstrap/interpreted/omega-executable/run.sh --controls
sh tests/bootstrap/interpreted/omega-executable/run.sh --controls-b
sh tests/bootstrap/interpreted/omega-executable/run.sh --controls-c
sh tests/bootstrap/interpreted/omega-executable/run.sh --controls-d
sh tests/bootstrap/interpreted/omega-executable/run.sh --controls-e
sh tests/bootstrap/interpreted/omega-executable/run.sh --controls-f
sh tests/bootstrap/interpreted/omega-executable/run.sh --controls-g
sh tests/bootstrap/interpreted/omega-executable/run.sh --controls-h
sh tests/bootstrap/interpreted/omega-executable/run.sh --controls-i
sh tests/bootstrap/interpreted/omega-executable/run.sh --controls-j
```

The ordinary Epsilon controls reuse one compiler across eighty-three
invocations split among [controls.epsilon](../../conformance/omega/controls.epsilon),
[controls_b.epsilon](../../conformance/omega/controls_b.epsilon),
[controls_c.epsilon](../../conformance/omega/controls_c.epsilon),
[controls_d.epsilon](../../conformance/omega/controls_d.epsilon),
[controls_e.epsilon](../../conformance/omega/controls_e.epsilon),
[controls_f.epsilon](../../conformance/omega/controls_f.epsilon),
[controls_g.epsilon](../../conformance/omega/controls_g.epsilon),
[controls_h.epsilon](../../conformance/omega/controls_h.epsilon),
[controls_i.epsilon](../../conformance/omega/controls_i.epsilon), and
[controls_j.epsilon](../../conformance/omega/controls_j.epsilon): zero and maximum byte results,
folded integral arithmetic and explicit anonymous bitwise/shift refusals,
multiple declarations and selection of a non-first entry, duplicate names,
missing entry, out-of-range and oversized decimal values, an out-of-range
expression operand, overflow, underflow, division and modulo by zero,
out-of-width and overflowing shifts, an invalid unselected body, unsupported
path, `-` unary, and other expressions and return types, a
body containing only nested declarations, digit separators, malformed syntax, and successful
reuse after failures; then checked control flow: call statements and grouped
terminal calls between machines with exact emitted-tape assertions,
subjectless and literal-subject transitions into nested machines, folded
Boolean subjects through the comparison, logical, and `!` operators, a
folded Boolean refused at the `u8` terminal, carrier-class rejections at
operators (`&&` on anonymous numbers, `<` against a Boolean, `!` on a number, `==`
across classes), a forward-declared callee, nested-machine name
resolution, unresolved call
and state targets, call and transition arity mismatches, an empty authored
state, an unmatched subject, duplicate authored state names, an unreachable
statement after a transition block, receiver and `self` call paths, a supported
parameterized transfer, first-match selection in a multi-arm subjectless block, a
non-Boolean subjectless condition, an
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
| `main_ocreq.epsilon` | 19,043 | `bff0a5aa5df1c4fa0f47ab267d05fa91c48238c81674be5a525435b73e0f668e` |
| `controls.epsilon` | 3,539 | `16a4915fedac6e95a50cb2b9405c988d995935146d0552606b698d3f52be8740` |
| `controls_b.epsilon` | 2,804 | `28701862d665951b2e432c58c77b7b1bcbc9d2b8ae53417f95bc76fa19908c6e` |
| `controls_c.epsilon` | 2,834 | `1ec2b16b0d97631bc88f23c0d39c417e6184875c1178913fba4629b815fc3465` |
| `controls_d.epsilon` | 2,852 | `b54adbcd30b9f192217b89b96efd3b548443a6e9426bb7891c98845fa89e8adb` |
| `controls_e.epsilon` | 2,795 | `881feab7746fe17185dfa163629b13c788274891f43c9ab7d72d1f07a3272d72` |
| `controls_f.epsilon` | 4,465 | `17c38612a596bd23ec8bee9f48a43e9bf6591595ba7ec819122acd9692033fc2` |
| `controls_g.epsilon` | 3,175 | `c3fb524c98a52ad999832af0b7525e75f98900ec69b6f6ac965348a3a20da137` |
| `controls_h.epsilon` | 4,385 | `87738a1eb03678fb979c48b2770c91a9d03a2f0539c19a49101233b6e786d5c3` |
| `controls_i.epsilon` | 3,754 | `5b1181c8e56caf2e1ff5ca4d1a7f4b61f0f16ba4488a645418985a73b8b9c6f4` |
| `controls_j.epsilon` | 5,585 | `d0fe765d9d0efa6aab8ab8b0444e87571b005b25d657572753b3e06bc915c4af` |

`tools/bootstrap/omega/compiler_env.sh` checks every entry identity before
each packing and `tests/bootstrap/interpreted/omega-identity.sh` covers the refusals. The
customer is the bound compiler closure followed by the selected entry. Each
execution reports its concatenated size and digest; those observations do not
introduce a second table of source identities.

## Execution boundary

This is a component-level source-to-executable regression, not the final
[standalone compiler interface](../../../../wiki/spec/build/compiler_request.md).
It uses the existing private Epsilon execution observation format. It does not
implement package resolution, Build evaluation, general ProgramEntry
selection beyond the target's bound `main` name, general Omega checking, or a
compiler-refinement proof. `main_ocreq.epsilon` is the gate's entry: the real
OCREQ/OCOUT route for the `alpha_bootstrap_tape` slice, bound like every
other selectable entry. `main.epsilon` remains bound for the `--diagnostic`
raw-source lane.

The additional [controls_values.epsilon](../../conformance/omega/controls_values.epsilon) customer uses
an internal selected-entry boundary to check parameter/local/call lowering and
negative cases, then publishes length-framed artifacts for actual Alpha
execution. `python3 tests/bootstrap/compiled_vm/source_test.py --omega-values`
constructs its source-only semantic producer path. Each invocation stores the
exact packed compiler, complete customer, request and observation; historical
measurements remain bound to their original sources. This customer is separate
from the public OCREQ admission checks.


The source-built peer's `--omega-utf8` customer extracts `DecodedScalar` and
`decode_scalar` verbatim from the real lexer, alongside the real SourceUnit
append/read declarations. [utf8_controls.omg](../../conformance/omega/utf8_controls.omg) checks 28
valid/invalid UTF-8 boundaries and an absent large index. The companion
[controls_arithmetic.epsilon](../../conformance/omega/controls_arithmetic.epsilon) checks arithmetic
limits, six abnormal overflow/underflow traps, left-value capture across a
mutating right operand, short-circuit suppression, reference forwarding and
refusal recovery. Wrapping controls exercise all 64 bits, preserve the left
value across a mutating right call, and reject mixed-policy operands and
implicit qualification changes. The nested crash controls cover declared
and inferred routes, disconnected helpers, cyclic transfers, contract order
and refusal recovery. The terminal-call cases exercise precedence,
grouping, nested calls, left-to-right argument evaluation, skipped and selected
crashes, loans, blocking markers and entry preconditions. An unsupported
indexed carrier returns a coverage refusal. An unknown name in a checked
scalar operand remains a source error. Each refusal leaves the tape empty
and unsealed. Transition predicates also exercise keyword-spelled local names,
lookup in an uncalled machine and Boolean literal selection.

The carrier-cast batches compare the combined and parenthesized domain additions,
execute a mutating operand once, and trap on overflow after preserving a full
unsigned 64-bit value.

Each batch uses a fresh evaluator process to bound cumulative allocation. The first receives
the full decoder source. Each refusal batch reuses one compiler instance and
ends with a successful compilation. The host stamps and runs every
published tape, distinguishing Linux SIGILL from a normal process exit.


Ordered subjectless Boolean predicates are also exercised by
`tests/bootstrap/independent/directory_names.py --case predicate-effects-ordered`
against an executable construction entry. The `predicate-*` cases check the
product's three-field `same_type_form` guard shape over scalar fields with each
answer-changing mutation, first-match order, once-only and short-circuit guard
evaluation, checked nested arrivals, and rejection of ill-typed unselected arms.
Every refusal requires an exact outcome and no Alpha tape. Coverage through a
wildcard or literal `true` is supported; more general coverage proofs return
`Incomplete`. The `sum-equality-*` controls exercise case comparisons with the
product's original enum declarations. Four execute the complete
`static_integer_is_valid` machine from a separate source unit, covering an
exact unsuffixed integer, an inexact integer, missing digits and a suffix.

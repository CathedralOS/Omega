# Epsilon-written bootstrap compiler

This directory contains the bootstrap compiler that must eventually construct
the Omega compiler in [`../../../source/`](../../../source). It currently compiles a
checked scalar and record subset into executable Alpha tapes. It does **not** yet compile
the Omega compiler source closure or close the Omega self-rebuild edge.

The source can run through the sibling [`../../epsilon/interpreter/`](../../epsilon/interpreter)
evaluator or the source-built compiled peer in
[`../../gamma/compiler/`](../../gamma/compiler). These are peer construction paths.
The intended remaining edge is:

```text
Epsilon execution + bootstrap compiler + Omega compiler source → first Omega compiler
first Omega compiler + same Omega compiler source              → rebuilt Omega compiler
```

The bootstrap compiler may support a subset sufficient for its successor, but
every supported facility must preserve Omega semantics. Unsupported forms fail
explicitly. The maintained Rust compiler is a development comparator, never a
semantic stage hidden inside the bootstrap route. Formal bootstrap correctness
certificates are outside current acceptance; ordinary language checks and
contracts still apply.

The stable artifact names and task IDs `D` and `C` in older records identify the
bootstrap compiler and Omega compiler respectively. Implementation and remaining
closure work are tracked in [`../../TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

## Compiler stages

The parser asks `scan_significant_token` for the next grammar token. The method
skips trivia, checks that the scanner advanced correctly, and distinguishes a
token from end of input and an internal failure. The scalar compiler checks
the parsed declarations and bodies and calls the Alpha encoder directly.
These stages share one Epsilon translation unit.

| Stage | Input and result | Implementation |
| --- | --- | --- |
| Tokenization | Given validated source bytes and a cursor, produce a token kind, span and next cursor, or a lexical diagnostic. | [Scanner](source-to-tokens/scan.epsilon) and [byte classification](source-to-tokens/byte_classes.epsilon) |
| Parsing | Consume tokens from each source unit and fill the parser's declaration, statement and expression tables. | [Parser](parser.epsilon) and [shared representations](representations.epsilon) |
| Declaration resolution | Resolve a name span to a declaration index using the requesting unit's package, module, imports and visibility. | [Resolver](resolution/declarations.epsilon) and [source set](source_set.epsilon) |
| Checking and lowering | Check supported bodies and select the Alpha instructions for their operations. | [Scalar compiler](scalar_compilation.epsilon) |
| Alpha emission | Encode instructions, resolve label fixups and validate the tape before publication. | [Tape encoder](alpha/tape.epsilon) |

The Rust compiler's front end also separates tokenization, parsing and name
resolution. Its later stages produce Psi. The Epsilon compiler's executable
product is an Alpha tape. [Rust pipeline](../../../source-rust/subsystems/compiler/source-to-psi/pipeline)

### Private helper arrivals

A private nested helper with no authored `requires` can infer one unsigned
parameter interval needed by fixed-array reads. The compiler checks its body
under that proposed condition, then revisits every ordinary and terminal arrival
before the owner may emit. Actual argument identities, representation-preserving
carrier removal, pure terminal guards and recorded mutation effects govern the
proof. A guarded field's declared upper bound can close `index < count <= extent`.

This provision covers same-owner static arrivals with an acyclic predecessor
graph. Public/free entries, explicit contracts, uncalled helpers and recursive
arrivals do not acquire inferred conditions. Ordinary calls currently transport
literal or existing entry facts. Broader copied-value and call-site inference
remains `Incomplete`. The constructor regressions named `private-arrival-*`
check these boundaries, including exact product declarations on the full
`SyntaxTrees` carrier.

## Retention inventory

| Retained file | Canonical role | Deletion condition |
| --- | --- | --- |
| `omega_compiler.epsilon.sources` | Canonical ordered membership, byte lengths, and SHA-256 identities of Epsilon-written compiler closure `D`. | Delete only when an adjacent compiler input format carries and verifies the same source-member custody directly. |
| Epsilon source members | The members separate representations, request/UTF-8 validation, lexing/parsing, source-set ownership and resolution, Alpha tape construction, scalar compilation, and outcome encoding. | Replace a completed component only atomically with an equally complete final Epsilon implementation and synchronized manifest identity. |

Target identity is an exact immutable compiler-invocation input. `build.omg`
under [the hosted application](../../../source/application/ocreq) does not declare a support
list or repeat target host/boundary policy; exact
multi-target requests fan out into independently checked child compilations.

The v2 construction entry interprets a ground application Build and the
empty activated core default. It checks the application role, the authored
`alpha_bootstrap\ProgramEntry` binding, and ProcessExit selections for the
six supported targets. Local Build helpers receive the caller's mutable Build
handle. Every helper body is checked. Only executed calls contribute a
selection, and each target retains its own provider. The file's own imports resolve each designated Product operand.
An incomplete activation stops before code publication. A completed pure
application compiles from its resolved entry declaration. The selected
terminal provider remains separate from the runtime's process-domain grant.

The default construction entry has no process domain. A standalone Alpha
embedding can pass its independently approved, immutable deployment to
`Main::construct_standalone`. The entire guest frame must match. The
constructor then executes the scoped Build, resolves its entry and selected
provider, checks each call's reach and emits Alpha HALT with the evaluated
signed i32 status. A different deployment refuses before publication.
The independent process-domain test builds an embedding with a fixed
deployment and checks the full status alongside the Linux seed observation.
[Standalone test](../../../tests/bootstrap/independent/process_domain.py)

A standalone deployment can also supply the distinct package roots of a
verified immutable Git snapshot. The covered application registers one Product
dependency with `Build::dependency(builder, dependency\from_source(Source::Path { location: "…" }))`.
The canonical location is relative to the application's root. Its target must
own the supplied dependency package, default alias and source bytes. The
dependency's own `Build::package` activation can select its provider. The
remaining package supplies the trusted core default. Build-only dependencies,
additional graph edges and overlapping package selections require further
coverage. To prepare this embedding, pass the snapshot, immutable commit,
request, output path and package roots in ordinal order to the origin verifier
with `--embedding FILE`. It checks every Product and Build file against Git.
[Origin verifier](../../../tests/bootstrap/independent/deployment_origins.py)

The manifest, not directory enumeration or path order, selects and orders D's
members. `mbx xtask source-closure` validates every
declared length, digest, source byte, path, strictly increasing stable
identity, and exact source inventory before concatenating bytes without separators.
The manifest materializes 30,798 lines / 1,217,811 bytes with SHA-256
`27f6b21f00093fee1f0592a6571fe90fb0b34529742a5fb3448cc3fed17f7a2a`.
The manifest itself is 3,249 bytes, SHA-256
`efd2d857e02de1494035e4b8fe09de67306cebe5c838d76e02790486359010c6`;
`tools/bootstrap/omega/compiler_env.sh` checks both identities against every
materialization. The source-closure verifier checks the resulting packed bytes.
A digest binds source custody; it does not prove compiler correctness.
Earlier parser measurements remain historical observations of their original
source subjects, not measurements of the current closure.
The authoring files preserve one Epsilon translation unit and its packed
diagnostic coordinates.

The [source-to-executable gate](../../../tests/bootstrap/interpreted/omega-executable/README.md)
connects parsed scalar machines to checking and the shared Alpha encoder.
Its public request route validates OCREQ shape and re-walks the bound subject;
the complete OCREQ semantic admission phases remain unfinished as detailed below.
The direct `compile(source, entry)` boundary is an internal compilation customer,
not a replacement public request protocol.

The internal `compile_set(source, entry_unit, entry)` customer accepts a bounded
source set: at most 64 package identities, 256 direct dependency aliases and 512
source units. Each unit has invocation-owned package membership, an exact
source interval and a logical package-relative path. The parser keeps each
parent directory component for name resolution and refuses a legacy `module`
header. Legacy directory imports share a namespace across sibling files. Types and calls retain
their exact declaration identity across units. Source-order changes do not
select a different imported declaration.

For multi-unit construction the compiler parses each whole file in a fresh
syntax arena. Free machines may take or return by-value scalars, records or sums,
and may contain nested states. A machine without a result completes with Unit.
The declaration directory retains names, visibility, owner files and disjoint
ranges for data declarations, fields, sum cases, payloads, machines, states
and parameters. Up to 16,384 state labels can span a construction request.
Each file still parses at most 4,096 states.
The parser admits at most 16,384 parameters in one file. A multi-file request
can exceed that total. Machine roots keep permanent argument slots across
files, and nested machines reuse the remaining slots while their file is
loaded. The root parameters across the request plus the largest nested
parameter count in any one file must fit in 16,384 slots. Emitted code gives
different nested machines distinct argument storage so a call preserves the
caller's values. Parameter ranges, source coordinates and declared effect
bounds survive the release of the file.
A state body, its lexical scope, and its checked terminal instructions belong
to the current file. Calls across files resolve a machine root; nested states
remain visible through their lexical owner.

A fixed-array literal uses the declared element type and count. Each initializer
runs once in increasing index order. Byte elements occupy one byte, while word
and copy-record elements use their checked stride. A policy-qualified element
needs a value with that policy. The product loan checker uses one explicitly
qualified zero to initialize both nine-element traversal arrays. An ordinary
call can lend a local fixed array to a shared or exclusive scalar-array
parameter. The callee indexes the caller's storage. Separately written types
must agree on element type and extent, including across source files. Calls
can forward an incoming loan, and a terminal state transfer can carry one of
its owner's incoming loans. A fresh loan of a local at that transfer still
receives an incomplete outcome.

Each file's imports resolve before signature or body checking. Record and case
payload fields retain the scalar type or nominal declaration resolved in their
own file. A fixed-array field also retains its logical count and resolved element
type, including a nominal element declared in a later file. The layout checker can then follow forward and nested records across files and
detect a by-value cycle. Public interfaces are checked against their owner's
imports. The compiler gives parameters permanent addresses. Bodies reuse
local syntax rows and get fresh storage addresses, so a callee can use the
same local row number while its caller's value remains live. Call cycles and
effects are checked before publishing the tape.

Shared byte-slice parameters carry a borrowed range and its live length across
a file boundary. Sum predicates and machine entry requirements resolve in the
declaring file. The checked case bounds, parameter bounds and crash ceilings
remain available after that file's syntax is released. Nested array fields,
array-valued parameters and results, other references and clauses naming
services still use the pooled checker. The single-unit public resource limits
remain in force.
[Per-unit compilation](construction/units.epsilon)

A file import binds its explicit alias or its filename stem. For example, the parser uses
`use representations/tokens/numeric.omg;` and `|numeric|::IntegerSuffix::None`
to select the suffix case from its token file. The constructor checks the
whole logical filename and package visibility before selecting a declaration.

Legacy imports are file-local. Item imports select declarations; namespace imports
bind a namespace without importing its ordinary machines as bare names.
Repeated imports of one identity are allowed, while competing imported names
refuse. Ordinary package-qualified names use the requester's own package or
a direct requester-local dependency alias. The reserved `omega\language\core` mount
selects the package with the invocation's core role. Both routes enforce public
visibility across packages. Public field and call-interface types cannot expose private records. The
current subset handles directory namespaces, free machines and named
records; it does not add attached declarations, reexports, overload selection,
or full build/request admission. The single-view API is the one-unit case.
Every supplied declaration is checked, including unused units and bodies.
Run `python3 tests/bootstrap/independent/directory_names.py CONSTRUCTOR` to
check directory imports, sibling lookup, path validation and legacy packets
against a constructed executable. Successful cases execute the emitted Alpha tape.

The construction application is [construct_omega.epsilon](../construct_omega.epsilon).
It reads package, direct-alias and unit rows followed by a shared 8 MiB arena
for source bytes, logical paths and names. Package roles distinguish ordinary packages from the explicitly
trusted core and standard library; spelling alone grants no such role. The
selected entry is a source-unit ordinal and name span. This internal input
context is separate from OCREQ package acquisition and admission. A successful
invocation writes Alpha tape; a refusal writes its source coordinate and exits
nonzero. The implementation remains under construction; Epsilon source
acceptance alone does not establish compilation of the Omega compiler closure.

The emitted subset retains a nullary `u8` program entry. Free and nested
machines may use plain `u8`, `u32`, `i32`, `u64`, `bool`, `u32 in Trapping`,
`u64 in Trapping`, `u64 in Wrapping`, copy records, copy sums with supported payloads,
and Unit results. Omitted results independently mean Unit; a nested declaration
never inherits its enclosing machine's result. Initialized and zero-established
locals retain their declared types, and mutable bindings permit checked stores.
It supports
resolved ordinary free calls, nested argument calls, scalar comparisons,
short-circuit Boolean operations, and parameterized same-owner tail transfers.
Boolean transition subjects may depend on runtime values, with Boolean or
wildcard guards, a comma between arms and an optional trailing comma. An arm
can transfer to a machine or produce a value. Booleans, integers, local names
and qualified sum cases with or without named payloads use the ordinary result
checker and Alpha emitter. In a subjectless transition the compiler checks every
condition as a Boolean and checks every target, including targets after an
unconditional arm. At runtime it evaluates conditions in source order until
one succeeds. A wildcard or literal `true` establishes coverage in this subset.
An operand of a transition subject may use a local with a keyword spelling,
including `target`. The ordinary name resolver checks that binding.
Boolean literals and `self` retain their expression roles. Match expressions
and the type spelling `Self` remain outside this transition-subject slice.
Constant transition subjects retain their specialized code path.

The bootstrap compiler implements intrinsic equality and inequality for values
of the same payload-free sum. It reads the left case before evaluating the
right operand, which can call a machine or change the left operand's backing.

An unrestricted `[copy]` sum uses a private eight-byte case tag followed by
storage for its largest admitted payload. The compiler checks each field's
type, copyability and bounded size. Named initializers evaluate in source order
and must initialize every field of their selected case once unless a final
`..` requests zero for omitted fields. Record and case literals check this
marker separately at each nesting level. Zero storage establishes the first
case with zero-initialized payload fields.

Case transitions evaluate the subject once. A braced arm can bind a field by
its own name, rename it with `field as name`, or waive it with `field as _`.
An explicit `..` waives remaining fields. The checker resolves the source
field within the selected case, checks duplicate fields and binding names, and
exposes each binding only in that arm's target. A fresh affine sum can be
returned or held in a local. A terminal transition can inspect its tag and
bind copyable payloads. Passing an existing affine local by value and
payload equality guards still need broader checking.

The standard `ByteRead` declaration permits `Eof` and requires the signed
payload of `Byte` to lie between 0 and 255. The checker resolves each
case against its owning declaration and checks both endpoints on the same
payload field. Every declared case must occur exactly once in the predicate.
Each construction establishes the selected case’s range. Pattern matching
carries this range into the selected arm. A machine guard or an explicit
entry requirement can establish a signed bound. A signed upper bound gives
no evidence for the lower bound. [Case requirements](checking/case_requirements.epsilon)
[Payload binding rules](../../../wiki/spec/language/patterns.md) and
[runnable selection](../../../tests/bootstrap/conformance/omega/payload_bindings.omg)
and [zero omission](../../../tests/bootstrap/conformance/omega/payload_omission.omg)

A signature census runs before bodies resolve calls or transfers. Unsupported
overload families refuse; identical admitted signatures reject as duplicates
regardless of binder spelling. Every body and arm is checked, including those
unreachable from the selected entry. Machine clauses beyond the requirements
and the crash and blocking ceilings described below, receivers,
other runtime carriers/policies, arithmetic beyond the checked unsigned families below,
and other dynamic subjects remain explicit coverage boundaries. Nested declarations use canonical `machine`; the retired `state`
alias is no longer accepted. The spelling `state` remains an ordinary identifier.
Expression calls to visible nested declarations use their own parameter and result
signature, entry requirements, effect acknowledgements and checked call loans.
Lexical helpers take precedence over a free machine with the same name; a local
value binding still takes precedence over a helper. Ordinary nested call statements
remain an explicit coverage boundary.

Each machine owns the helpers declared after its executable body. Terminal
transfers search the caller's children and then each enclosing declaration
scope. Sibling branches may define different helpers with the same name.
Parameters and locals belong to the individual invocation. A child receives
an enclosing value through an explicit argument. The parser admits up to
4,096 state bodies per file. Construction can address 16,384 states across files.

A terminal transfer naming a visible global is `Incomplete` in this same-owner
subset; only an absent target is reported as missing.

Anonymous constant arithmetic retains its mathematical meaning. The bounded
folder supports integral `+`, `-`, `*`, and `/` when every intermediate lies in
`0..255`, plus comparisons and Boolean operations. This is an implementation
coverage bound, not an Omega carrier inferred from the destination. Fractional,
negative, and wider intermediates return `Incomplete`; division never silently
truncates. Anonymous remainder and width-dependent bitwise/shift operations
also return `Incomplete`. Typed operations need their own checked lowering.

For plain `u64` subtraction, the compiler checks both operands as `u64`
and proves the result is nonnegative. In `capacity - used`, a declared
ceiling on `used` can establish the proof. In `value - count`, a floor on
`value` can establish it. Subtracting zero preserves every bit of a `u64`.
An unsigned difference that returns is at least zero and at most its left
operand. Calls and state transfers can use a checked ceiling on that operand.
A Trapping subtraction still checks for underflow on Alpha.
The interval checker accepts nonnegative literal endpoints through
2,147,483,647. A wider literal cannot supply an endpoint from its low word.
A direct out-of-range `u8` literal and division by zero reject. Full rational
folding, broader explicit landing, and other typed runtime operations remain work.

Private static storage separates parameter, local, pending-argument and
expression-scratch cells within a checked 1 GiB backing window
`[0x20000000, 0x60000000)`, above Alpha's downward-growing return stack.
`omega_storage_base()` and `omega_storage_capacity()` supply this private map
to every backing-address emitter and layout/allocation check, including borrowed
byte-view descriptors. The Alpha tape limit remains **16,777,212 bytes**.
This map describes the emitted Omega program, not the Epsilon evaluator or
Gamma runtime that executes the compiler; those have separate memory profiles.

| Region | Half-open address range or origin | Admission |
| --- | --- | --- |
| Tape and inline data | `[0, L)`, `L <= 0x00fffffc` | Existing encoder and seed materializer check the unchanged raw tape limit. |
| Alpha return stack | Origin `0x10000000`, grows downward | Alpha checks each complete eight-byte push and pop against its memory extent. |
| Checked value backing | `[0x20000000, 0x60000000)` | Nonzero rounded extents and the accumulated arena must fit `0x40000000` bytes. |

The backing window is disjoint from tape and stack, and its exclusive end is
below signed `i32`'s maximum and AlphaBootstrapV5's `0x2000000000` memory extent.
Array products are admitted by `count <= capacity / stride` before multiplying;
record/payload growth and cell allocation use `extent <= capacity - used`
before adding. Word rounding remains within positive `i32` arithmetic. Every
checked index plan repeats the extent/product check before emitting an address.
These are private compiler limits; Omega's array, provider and ownership rules
are unchanged. Layout or arena overflow retains its existing `Incomplete`
refusal before tape publication.

The normal request controls in
[`directory_names.py`](../../../tests/bootstrap/independent/directory_names.py)
cover the exact and first overflowing array, record, payload and arena extents;
a product that would wrap `i32` multiplication; independent live regions; and
borrowed-byte descriptors after relocation. Its complete `SyntaxTrees` customer
retains the product declaration's fields, bounds and data dependencies. It
reserves two 45,156,664-byte owners and checks their final word and an earlier
constrained field through separate calls. The arena-boundary customer reserves
an unused parameter region, so testing a 1 GiB reservation does not execute a
1 GiB initialization loop. Source-owned seed observations require a replay
against the exact selected compiler identity.

Each call evaluates arguments left to right into
its own pending cells before copying them into destination parameters, including
same-state transfers. This preserves
earlier arguments and caller locals across nested calls. A whole-owner call
DAG collects free-machine calls from every state. Nested expression calls retain
exact declaration identities in the state-invocation graph. For each such call,
a bounded reverse walk through ordinary calls and terminal transfers refuses
any route from the callee back into its suspended caller. Each state is queued
once per walk. Terminal-only cycles remain admitted, including within a called
helper. Both checks cover unused declarations as well as the selected entry.
At most 16,384 distinct state invocations can be suspended by ordinary calls.
No admitted source operation converts these private addresses to numeric values.
An `i32` value retains its sign through local storage, parameters, and returns.
Comparisons between `i32` and `i32`, `u8`, or `u32` use signed Alpha word order.
Comparisons between unsigned words bias both operands by the high bit first.
The construction writer can widen any output byte with `as i32`. A narrowing
from an unrestricted `u32` has no such proof. The core output helper
`(value & 0xFF) as u8` uses its mask to prove the result is within `0..255`.
For `i32`, a literal mask of contiguous low ones admits either operand order.
The Alpha emitter evaluates the other operand once, then normalizes signed
remainder modulo `mask + 1`. Integer expressions accept binary, octal, decimal
and hexadecimal spellings and digit separators. An expression with an
explicit `u8`, `u32`, `i32`, or `u64` suffix
retains that carrier through storage,
calls, domain casts, and returns. A mismatched destination rejects at the
literal. Other valid integer carriers retain their spelling in the parser and
remain outside executable coverage. Signed literals cover
`-2147483648..2147483647`. The compiler accepts unsigned literals through
18446744073709551615 for `u64` and emits both words in one Alpha immediate.
Narrow integer predicates and index bounds require the complete magnitude to
fit their proof representation. For plain `u32` addition, the compiler tracks which bits may be set
in each operand and admits disjoint masks. It admits multiplication by a value
whose only possible values are zero and one power of two if shifting the other
mask cannot discard a bit. These proofs apply to byte widening and immutable
scalar bindings. Mutable bindings and unknown word inputs retain the full
carrier mask. Both operands execute once in source order.
Plain `u64` addition by a nonnegative literal is admitted when
checked upper bounds prove the complete result at most 2147483647. The emitted
word addition is then exact; unproved plain arithmetic remains unsupported.
Qualified `u32 in Trapping` supports addition, subtraction and multiplication;
qualified `u64 in Trapping` supports addition and subtraction. Each selected
core operator checks overflow or underflow and realizes an abnormal Alpha Trap.
For unsigned subtraction by a nonnegative literal, a checked lower bound on the
left operand can discharge the underflow route. Both operands still contribute
their own effects and Alpha still performs the runtime check.
Qualified `u64 in Wrapping` supports addition, subtraction and multiplication
modulo 2^64. These operations introduce no crash effect and retain the policy
on the result. The qualified operand selects the family; a same-carrier plain peer is allowed,
and a receiving annotation never inserts qualification. Trapping `u64` multiplication
and the remaining runtime operators decline explicitly. Named checked-value
variants retain the selected realization; operands evaluate once, left to right,
with private scratch preserving a captured left value across a right-hand call. Comparisons retain carrier/policy identity rather
than treating every word as interchangeable. Ordinary arguments, locals and
results never acquire or drop a domain implicitly: `as +Trapping` and
`as -Trapping`, or `as +Wrapping` and `as -Wrapping` on `u64`, express the
supported domain changes. Core comparisons admit a
plain peer of the same carrier, as specified by the policy-domain operator
family; that does not qualify the peer. Carrier casts accept plain `u8`, `u32` and `u64` identity,
anonymous landing within the supported literal range, and exact widening
between these carriers. On `u32` or `u64`, `value as T +Trapping` checks
the carrier conversion and then the destination domain, evaluating the
operand once. A value already qualified by Trapping must first remove it.
Narrowing and unsupported domain establishment remain coverage refusals.

This closed slice projects the primitive operators and the fixed core policy
comparison and arithmetic families specified by [numeric values](../../../wiki/spec/language/numeric_values.md#policy-domains);
it does not claim general
operator-declaration or package/core admission. Imports, custom operators and
user domains remain unsupported. Core comparisons publish `crashes none`,
including on Trapping operands. Evaluating either operand can still call a
machine that publishes `Trap` or perform Trapping arithmetic. The compiler
propagates those effects through calls and terminal transitions.
Each nested machine can declare its own unguarded crash ceiling. A disconnected
helper still satisfies its own contract and contributes no route to its owner.
An unguarded `crashes Trap` permits that route and publishes it to callers.
Private checked bodies infer an omitted ceiling. Public bodies with an omitted
ceiling and bodies declaring `crashes none` must discharge it. An explicit
`crash Trap;` terminates the current activation abnormally, using Alpha's
division-by-zero trap. The compiler checks unused declarations as well as
the selected entry. A forbidden unconditional first crash in an unconstrained
entry rejects, including an uncalled nested entry. Possible routes needing a path proof still return Incomplete,
as do guarded clauses and Abort. Full product effect admission remains open.
[Effect admission](../../../wiki/spec/language/effects.md)
A bare `blocks` or `blocks true` publishes a possible block on the machine
that declares it. A public machine with an omitted bound and any machine
declaring `blocks false` promise that their bodies cannot block. Private
machines, including nested helpers, infer an omitted bound. The compiler
propagates published ceilings back through ordinary calls and same-owner
terminal transfers until it reaches every caller, including transfer cycles.
An unentered nested helper contributes nothing to its owner's bound. Its own
body must still satisfy its contract.

The incoming-edge table reserves 49,152 rows with four 32-bit integers
apiece, or 786,432 logical payload bytes. Three 16,384-entry integer arrays
hold incoming list heads, per-state effects and the work queue, totaling
196,608 more logical bytes. Evaluator memory measurements include its
representation of these tables and the rest of the compiler.

Every call to a possibly blocking callee needs its own `block` acknowledgement,
including a call in an argument or a transfer in an unselected arm.
Redundant acknowledgements preserve the caller's bound. The existing Alpha
calls and jumps implement both forms. Global terminal calls, suspension and
host service admission remain outside this executable slice.
[Call-site acknowledgements](../../../wiki/spec/language/effects.md#call-site-acknowledgements)
and [terminal calls](../../../wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms)

Visibility uses canonical `public`; retired `pub` is an ordinary identifier,
not a declaration modifier.

A fixed-array type identifies its element and declared length. Plain arrays
occupy type IDs in `[200000, 300000)` and Trapping arrays in `[300000, 400000)`.
Within either band, rows below 16,384 refer to live parser type syntax.
For an array in a retained nominal declaration, the row is 16,384 plus its
field ID. That field retains the resolved element type and logical count after
the compiler releases the declaring file's syntax. Layout, indexed access and
borrowed byte ranges all consult those same two values.

Records retain declaration identity, field qualification and copy/affine
multiplicity. In the private Alpha layout, a scalar cell occupies one
eight-byte word. A fixed array of plain `u8` stores one byte per element.
Its indexing instructions read or write exactly one byte. The whole array is
rounded up to a word boundary so record copies and zeroing can use whole words.
Bounds checks use the declared element count. The `SourceUnit` byte array
occupies 128 KiB and the construction application's byte array occupies 8 MiB
inside its `ConstructionInput` record. Layout recursion is bounded
at 64 steps. Compound extents and the complete private storage arena are
bounded by the 1 GiB backing window described above. By-value cycles,
linear/opaque/authority-bearing types, stored references, selected cleanup and
unsupported domains decline before allocation. An intact affine local with no
contained cleanup may leave scope; it is never implicitly copied.

The first default-domain solver admits direct scalar-field `<=` literal bounds,
with the literal checked against the field's carrier. Zero establishment proves
these predicates recursively. Every field store must preserve the declared
bound immediately, using the same unsigned upper-bound evidence as indexed
access and explicit entry contracts. Unknown predicates and temporary invariant violations decline. A checked
record copy preserves all nested field validity. Calls retain only declared
validity, not stale stronger facts about fields they may mutate.

Copy records support exhaustive literals, nested field projection, owned local
storage, parameter/result transport and captured-place assignment. Literals
evaluate their fields in authored order; arguments evaluate left to right and
copy values into distinct staging storage before the next call can overwrite
its result. No affine record moves or partial initialization are admitted yet.
Fixed arrays may be retained, zero-established and copied inside these records.
Fixed arrays of `u8`, bare `u64`, and copy records support indexed reads and
whole-element writes through a checked core operation plan. The plan retains the concrete
element type, layout stride, array length, collection, selector, access mode,
parent backing and bounds evidence. A call returning a copy record or an indexed record can supply a named field
in an rvalue. Each field suffix retains its base expression and name span. The
compiler resolves the field against that record declaration and applies its
checked field offset. Whole-record stores copy exactly one element while the
loan covers the parent array. Explicit shared or mutable removal of the array
shell's `Trapping` domain preserves element types
and closes its empty restoration debt. Reads use shared views and writes use
mutable views; other reference attenuation remains unsupported. A bare `u64`
array local without an initializer uses zero-initialized storage and the same
checked selectors and element stride as a record field.

A machine taking `@[u8]` can read the live length of a shared byte slice.
A literal index needs a proved lower bound on that length. A scalar index
can also carry the declared requirement `index < bytes.length`. A half-open
endpoint can use `index <= bytes.length`; it still needs a strict bound for
a byte read. Two parameters with the same plain `u8`, `u32` or `u64` type
may require `start <= index` or
`start < index`. At each call or transfer the compiler substitutes both
actual arguments. Literals, pure selected comparisons and preserved entry
requirements can establish the relation on that exact pair. A true conjunction
can supply either of its comparisons. With `index < bytes.length`, the plain
`u64` successor `index + 1` cannot overflow and can reach the endpoint.
Reading at that successor requires a further bound. A preserved strict pair
`unit < limit` likewise establishes that `unit + 1` fits its unsigned carrier. Callers can borrow a whole packed byte array or a half-open subrange with literal endpoints,
including an empty range. The descriptor carries the original address and
the live length in two words. The caller retains the shared loan through
argument evaluation. The storage test forwards the final two bytes of the
construction input and reads the last byte through a nested call.
[Shared byte slices](shared_byte_slices.epsilon)

A shared incoming slice can also be narrowed with pure, checked `u64`
endpoints. The compiler proves `start <= end <= source.length` for that exact
slice; omitted endpoints mean zero and the captured length. Equality admits
empty views at the end without authorizing a read. The emitted descriptor
retains the same backing, evaluates start before end exactly once, and stages
before any callee parameter is overwritten. Nested transfers may narrow an
incoming shared loan but cannot introduce local array backing.

Mutable slices, effectful or inclusive range endpoints, other domains and
structural equivalence of separately spelled array value types remain outside
this profile. Dynamic ranges do not yet supply a derived length to a callee's
entry contract; that unsupported proof cannot become an assumed fact.

`tests/bootstrap/independent/shared_subranges.py CONSTRUCTOR NEW_DIRECTORY`
runs normal closed-set requests, then executes accepted tapes on Alpha. It
covers endpoint order, backing identity, empty/open ranges, argument staging,
tail reuse and refusal twins. `--product-helpers` additionally requires all
three unchanged Build-path helper callers to execute; this remains a separate
acceptance while later guard/call coverage is unfinished.

The storage test runs the production `path_length` on two nonzero `SourceRange`
elements of the production `SourceSet`. It checks a copy taken before a write,
a keyword-spelled field and the field beside the last element. It evaluates
both capacity differences from `source_set_add` at the boundary and with an
extra source byte or path byte. The unit-count store from `source_set_add`
uses the proved upper bound of its Trapping sum, including 511 + 1 at the
capacity of 512. Alpha still performs the checked addition. The complete
23,348-byte `source.omg` passes the layout of the real `SourceSet`. It
also accepts the three record references of `source_set_add`.
It reads `path` as a parameter in the nested helper even though a sibling
machine has that name. The compiler recognizes the shared slice parameter of `span_spells`.
It checks the nested reader's `index < spelling.length` requirement. The full
file passes the bodies of `source_set_add`, `add_package`, `add_dependency`
and `store_name`. Its next coverage refusal identifies `source_set_byte_or_nul`
at file byte 14,466. [Storage test](../../../tests/bootstrap/conformance/omega/indexed_record_fields.omg)

Index bounds come from literals, immutable snapshots, declared scalar-field
upper bounds, or explicit checked machine entry requirements. The requirement
profile supports `true`, conjunction, and unsigned formal or projected-field
bounds against a literal using `<`, `<=`, `>` and `>=`. Each retained entry bound
records an inclusive minimum and an optional inclusive maximum. Every call and transfer must discharge the exact target
requirements; unsupported predicates cannot disappear. Pure Boolean guards
establish bounds at nested arrivals, with pure actual expressions required for
contract-bearing calls. No private requirement is inferred from a later access.
An overlapping write or exclusive call invalidates the entry version. Call
mutation ranges are retained before checking the next operand, including within
the same expression; disjoint sibling ranges preserve their evidence. When a transition condition calls a
machine, its target may use independently established bounds that survived
the call. The checker discards proof derived from the condition itself and
checks retained writes before reusing an entry bound.
The collection is captured before
the selector and the resulting destination before the RHS; selector mutation
cannot redirect a pending store. A scoped collection loan remains live during
selector evaluation and, for assignment, through the RHS. Whole-array overlap
is conservative: this slice does not yet prove separate element loans disjoint.

Shared and mutable record-reference parameters carry the original referent
address. An explicit `$source` copies a named reference's copyable record,
and `$target = replacement` writes the whole record through a mutable
reference. Both use the full referent's range for loan and bound checks.
A caller can borrow several owned records or their projected fields
in one call. The checker keeps earlier arguments' loans live while it checks
later arguments. It permits two shared loans to the same field and an exclusive
loan beside a disjoint sibling. Overlap with an exclusive loan refuses in
either argument order, including when a nested transfer forwards one
exclusive parameter twice. The referenced
root cannot be rebound. Nested transfers may forward the enclosing invocation's
original incoming reference as a bare matching parameter; its whole referent
loan remains live while later arguments evaluate. A local-backed reference
cannot enter this path, so repeated tail transfers cannot outlive reused local
storage. Ordinary calls also accept a bare matching incoming reference and
retain the same argument-order loan and mutation range. Escaping/stored references, implicit
shared lending and lifetime syntax decline. Mutation through a shared reference rejects. All these checks finish before
any executable bytes are published.

The `--omega-storage` customer takes exact SourceId, SourceUnit, ModulePath,
`source_unit_clear`, `source_unit_append`, `source_unit_byte_or_nul` and
`module_path_append` declarations from `source/subsystems/compiler/source_to_psi/representations/source_files/source.omg`, adds
`storage_controls.omg`, and executes the emitted result. The real private
helpers now state their existing guarded bounds explicitly and remove the
array-shell policy through checked shared/mutable views. The customer checks
clear's preserved byte, zero/final/full append boundaries, absent reads and
ModulePath capacity. It writes distinct values to the first and last two
bytes of the application's `ConstructionInput`, then copies a nine-byte array
inside a record and checks the neighboring field. Generic controls check disjoint mutation, captured indexed
assignment, original-reference forwarding and explicit scalar entry contracts.
Refusals cover missing/stale bounds, mutating guard operands, same-expression
call invalidation, overlapping argument loans and local-backed tail transfers.
The parser and compiler contain no special cases for these helper names.

```sh
python3 tests/bootstrap/compiled_vm/source_test.py --omega-storage
```

The `--omega-control` source-built customer extracts the exact canonical
`is_identifier_start` and `is_identifier_continue` declarations from
`source/subsystems/compiler/source_to_psi/pipeline/source_files_to_tokens/lexer.omg`, adds the checked-in boundary-calling wrapper, compiles
them with this bootstrap compiler, and executes the emitted tape. Its companion
source-owned controls exercise parallel transfers, skipped looping operands,
nested comparisons, type/arity/range/grammar refusals and reset behavior:

```sh
python3 tests/bootstrap/compiled_vm/source_test.py --omega-control
```

Extraction and request framing supply bytes, not language meaning; no compiler
path recognizes these helper names or source shapes.

The source-owned `controls_values.epsilon` customer exercises parameter/local
semantics, nested argument calls, caller preservation, forward calls, state
transfers, and refusal/reset behavior. Run it through the source-built peer:

```sh
python3 tests/bootstrap/compiled_vm/source_test.py --omega-values
```

This reconstructs the source Gamma compiler and its successor evaluators before
running the manifested bootstrap compiler and executing its emitted artifacts.
It does not use Python to supply language semantics. Python frames and checks
bytes; the native Alpha seed and documented source/tape admission remain trust
inputs. The development comparator can also run these controls with
`python3 tests/bootstrap/compiled_vm/test.py --omega-values` and retains its separately
disclosed Python semantic dependency.

The scalar failure paths also record the canonical OCOUT outcome tuple
(outcome tag, coordinate space, code, coordinate, canonical ordinals, limit,
requested). Parser exhaustion projects onto its assigned resource code, tape
capacity reports under emitted-artifact coordinates, and source refusals
anchor at the retained span. The assigned `Reject` codes 1–12 remain
`malformed_request`, the eight lexical refusals projected from
the scanner's retained diagnostic, `duplicate_name` at the later
declaration's name, `missing_entry` with no coordinate, and
`integer_literal_out_of_range` at the refused literal's first byte. The
embedded projections now cover the full assigned inventory: the syntax
`Reject` codes 13–89 and the scalar checking codes 90–97 all anchor in
canonical source, and the `Incomplete` coverage provisions project
resource codes 15–23 onto canonical source and 24–25 onto the
coordinate-free space. Producers still record the unassigned marker for
the syntax, checking, and coverage refusals themselves, so the encoder
keeps refusing those recorded tuples until the recording side is wired. The
[outcome gate](../../../_recycling/tests/bootstrap/omega-outcome/README.md) exercises the
embedded tables, exact and refused frame encodings, bounded arithmetic, and
the recorded tuples through the selected evaluator.

The manifested members (`D`) exist but are incomplete; both descriptive compiler
tapes remain absent. The [standalone request contract](../../../wiki/spec/build/compiler_request.md)
owns OCREQ/OCOUT, including complete snapshots, subject binding, validation order,
and shared diagnostic selection. D's request decoder currently validates the
outer envelope and phase-1 declared-extent provisions in `outcome.epsilon`
plus the phase-2/6 subject and invocation field/tag shape pass in
`request_and_utf8.epsilon`; the semantic phases over the decoded fields —
package-key recomputation, canonical ordering, graph, snapshot, admission, and
commitment checks — remain unimplemented. The assigned OCOUT header,
outcome-code, coordinate-space, phase, and scalar-resource tables are embedded
there as projections with the canonical failure-frame encoder. The contract's
lexical `Reject` codes 2–9 and the named declaration, entry, and literal
refusals 10–12 are assigned and implemented in D; the syntax `Reject`
inventory, the remaining checking codes, and the `Incomplete` coverage
provisions are assigned in the contract and still need producer wiring and
the semantic phases over the decoded fields under OMEGA-D/OMEGA-C in
[the board](../../../TASKS_BOOTSTRAP.md).
A raw-single-file stdin convention cannot replace the sealed request.
[Build execution](../../../wiki/spec/build/execution.md) owns the admitted checkpoint,
one-way generated-source continuation, and dependency-bundle no-rerun contract.

Its retained Alpha encoder targets `AlphaBootstrapV5` directly: at most
16,777,212 raw tape bytes, targets below that exact extent, and at most 1,864,134
nine-byte-dominated fixups. These are outer compiler-resource bounds, not Omega
language limits, and move only with the common seed/checker profile.
The four direct address-writing paths admit offsets `0..16777211`, matching
that payload provision; finalization additionally requires every target to lie
on an instruction start inside the actual payload. The
[whole-member D customer](../../../_recycling/tests/epsilon/interpreted-omega-experiment/README.md)
checks exact forward fixups, sealing, undefined-label failure, and exact/adjacent
address admission through the selected Epsilon evaluator. It also constructs
a 79-byte echo/count tape through D's emitters and executes those exact emitted
bytes with the selected Alpha seed, covering all three symbolic operand shapes,
forward/backward targets, and call/return on empty, NUL, and high-byte input.
This is execution evidence, not a checked compiler refinement. It does not establish
full-size tape realization or a completed D compiler.

Epsilon cannot safely express a reusable validate-once source cursor: machines
and fields are public, while immutable views cannot be stored in data. `D`'s
parser therefore validates once and streams the same source through private
states of one canonical invocation. Its retained slices sequence empty,
trivia-only, ordinary `use path::member;`, basic `[public] data`, and ordinary
machine roots whose current bodies contain neutral path calls and simple
assignments.
The explicit slow [complete-D parser customer](../../../_recycling/tests/bootstrap/omega-parser/README.md)
checks this manifested implementation through the lower chain. Sum-valued
parser guards use Epsilon case transitions; scalar equality is not a sum test.
One root table preserves source order for imports, data, machines, domains, ordinary traits and boundary traits. Optionally
public ordinary machines retain an arbitrary name-like path, optional
parentheses containing a comma-separated state-parameter list, and a body from
the retained statement slice. An optional leading target selector is
retained as an exact span on the same machine row; selection and activation
remain later phases.

A data declaration can bind lifetimes and forward them through a named field
type. In `source/library/system/calling.omg`, the `Rejected` case of
`BoundaryPlanResult<'reason>` stores a `CallingPolicyRejection<'reason>`.
The parser retains each binder and argument in source order. The checker
rejects duplicate binders, unbound field lifetimes and applications with the
wrong lifetime arity. A valid declaration with explicit data lifetimes still
reports incomplete until the compiler checks loans carried through storage.
See [lifetimes and carried borrows](../../../wiki/spec/language/lifetimes.md).

A nongeneric domain declaration retains its visibility, name, carrier type,
and ordered predicate expressions. For example,
`public domain [u8]::Utf8 requires byte_observation\valid_utf8(@self[..]);`
in `source/library/system/calling.omg` retains the slice carrier and the call on
its borrowed subject. A declaration ending directly in `;` has zero predicates.
The parser rejects the retired subject binder and brace body. The compiler
reports incomplete at the domain keyword until it checks the carrier and its
proof obligations.

An ordinary `trait` retains its ordered typed static parameters. For example,
`trait Calling<const Policy: CallingPolicy> {}` preserves the parameter name
and declared type. The parser rejects repeated parameter names and the retired
`Policy satisfies CallingPolicy` binder. The compiler still reports incomplete
for an ordinary trait until it can check and publish its contract.
[Typed static parameters](../../../wiki/spec/language/generics.md#typed-static-parameters)
govern the source spelling. The installed `std/calling.omg` still uses the
retired binder. Its migration belongs to `TYPED-STATIC-PARAMETERS` in
[TASKS.md](../../../TASKS.md), alongside the product compiler's checks on callers
such as `Calling<LinuxX86_64CallingPolicy>`.

An ordinary `trait` retains each bodyless machine signature in
its declaring trait's range. `ConsoleByteOperations` and
`PosixConsoleRawOperations` in std use this form. The parser keeps ordinary
traits in a separate table from boundary services. Executable construction
reports incomplete coverage at an ordinary trait until its conformance is checked.

A nongeneric `[public] boundary trait`
can declare bodyless machine requirements. Each trait owns its name, visibility,
source span and ordered requirements. A requirement's name, parameters, return
type, predicates and effect clauses retain their original spans and handles.
Requirement names may repeat across overloads. Reach, invocation, crash and
blocking clauses use the ordinary machine-header parser. Each `invokes Entry`
on a free machine or trait requirement owns a distinct binding-name span. The
rows preserve source order. Checking an entry ceiling needs the binding and
its direction, so this compiler currently returns `Incomplete` on such contracts. Bare `terminates` on a
requirement retains the public eventual-termination guarantee. The checker resolves a memberless service to its exact declaration, including
its package and module. An ordinary call contributes its callee's published
reach. An omitted ceiling infers that reach through every checked call. An
explicit ceiling publishes its authored set and rejects an excess at the
contributing call. The construction suite uses the actual core process service
declarations across a package edge and runs the resulting tape on Alpha.
[Service checker](resolution/services.epsilon) ·
[Construction suite](../../../tests/bootstrap/independent/service_reach.py)

A boundary may declare one terminal operation taking an ordinary `i32` and
returning a fieldless record whose sole default predicate is the literal
`false`. The checker resolves its result under the boundary's source unit.
Its requirement publishes the owner's service reach, `crashes none` and
`terminates`. Both statement and expression calls check the single signed status argument.
Wrong arity or a mismatched value rejects at the call or argument. Calling
the uninstalled requirement with a well-typed status returns `Incomplete` at
its boundary keyword.

Like the product scalar lowerer, the isolated Alpha adapter accepts a
parameterless entry returning either an ordinary byte or Unit. A normally
returning Unit entry yields process status zero. An explicit `_ -> {}` (or
a guarded `true -> {}`) completes a Unit state and returns to its caller.
A state that owes a value rejects an empty-brace target. Entry selection grants no
stream or exit service. [Product entry result](../../../source/subsystems/compiler/source_to_psi/pipeline/checked_trees_to_scalar_operations/scalar.omg)

A construction package can also include the authored core
`ProcessExitPrimitive` and std `NativeProcessExit`. The checker resolves the
primitive to the exact public declaration in the toolchain core root and checks
its `satisfies` member, reach, result and termination. The named implementation
owns its member. The supported body makes one call to the resolved primitive,
passing its own signed parameter. Changing that value or shadowing the primitive
leaves this forwarding subset. The v1 package installs neither a provider nor
a process domain. Calling the requirement, primitive or wrapper still refuses.
[Provider checker](resolution/providers.epsilon) ·
[Standard provider](../../../source/library/system/process_exit.omg)

The same proof of an empty default domain applies to ordinary checked
machines. A machine returning such a type can end in a checked Trap or
transfer to another non-returning machine. Calls still check their arguments
and propagate blocking, reach and crash obligations. On Alpha the transfer
jumps to the checked callee without pushing a return address. Zero
initialization and record literals cannot establish a value of the empty
type. [Published no-return](../../../wiki/spec/language/state_contracts.md#published-no-return) ·
[Uninhabited types](../../../wiki/spec/language/data_and_literals.md#the-uninhabited-type-never)
[Core ProcessExit](../../../source/library/core/process_exit.omg) ·
[Process-exit contract](../../../wiki/spec/language/process_exit.md)

A source-ordered machine-clause ledger retains exact
non-generic `satisfies Trait::requirement` bindings and nonempty `reaches`
ceilings over comma- or plus-separated service identifiers. It also retains
the cause or empty spelling of each unguarded crash clause. A satisfying clause
may additionally own the exact `via`, `Binding`, and `CompilerIntrinsic` spans
for the payload-free closed external binding. That form terminates one bodyless
external-leaf machine with its canonical empty implicit entry. The ledger also
retains the private ranking witness `terminates by <path> -> <View::Path>;` as
separate source-ordered subject and view paths plus exact `by` and arrow spans;
it does not confuse that form with bare `terminates;`, which authors the public
eventual-terminal guarantee. Tuple or non-path subjects, an omitted or
argumented view, ranking ranges, the bare guarantee on ordinary machines,
generic satisfies arguments, aliases, other external binding cases, and empty or
installation-bound reach rows remain incomplete. The parameter list
retains canonical optional `const` and leading
`mutable`, consuming or borrowed `self`, and shared/mutable/write-only
binding-reference forms. Omega writes these as `@T`, `@mutable T` and
`@write T`. Borrow expressions use the same `@` marker. Binary `&`
computes bitwise AND.
Non-receiver parameters retain `name: Type`. Non-receiver parameter types, data
fields, case-payload fields, and immediate machine return types share one engine
for bare named/Self/Unit, outer references, unqualified domains, inclusive
literal ranges, and nested fixed arrays and slices. References may retain an
exact explicit lifetime, and general `Self` uses the same `SelfType` base as
receivers. Every parameter row records its canonical const/mutable/self flags;
the owning implicit or explicit state owns the optional return node.

The retained nonempty-body slices include ordinary semicolon-terminated call
statements without acknowledgements, evidence arguments, or result discard,
and terminal expression statements over the retained primary path/literal slice,
plus a restricted canonical transition core.
Each call owns a flattened receiver-member span, an exact target
member, and a contiguous span of argument expression handles. Its current
value arguments are exact name/self paths, booleans, nonnegative
decimal integers, or string tokens represented by tagged expression nodes;
the same lane accepts shallow named struct literals over those primary field
values. Zero arguments and trailing commas are supported. Calls may also own a
separate nonempty `::<...>` list of path-only static operands. Each call keeps
its own list even when a value argument is another generic call. A plain
`<` enters the comparison grammar. Qualified
paths are retained exactly; const arguments, evidence projections, nested
static applications, lifetime arguments, and empty or trailing-comma static
lanes remain incomplete. Ordinary assignments retain
separate target and value handles. Their current target grammar is a self/name
place path followed by zero or more indexes, each with a path, decimal, or
canonical range over the retained path/decimal bound slice; current values are
self-member paths, qualified-name paths, booleans, nonnegative
integer literals, or strings. Integer
nodes retain the complete source span and the start of any explicit carrier suffix;
strings retain the exact token span and scanner-proven decoded byte length
without a decoded-byte mirror. Assignment values, local initializers, ordinary
call arguments, and transition subjects share one bounded precedence reducer
over `||`, `&&`, equality, comparison, single-domain contextual `in`, `|`, `^`,
`&`, `<<`, `>>`, `+`, `-`, `*`, `/`, and `%`. Every ordinary operator
materializes a source-ordered binary node without evaluation or type guessing;
membership retains its value and domain path directly in the tagged expression
row. Domain composition after `in` remains parser-incomplete and cannot fall
through to the integer bitwise lane. Once formed, an unparenthesized membership
may continue only into comparison, equality, or logical tiers; grouping is
required before a tighter operator can apply to its Boolean result.
Parentheses delimit reduction frames and preserve their exact transient source
extent without manufacturing a group syntax node. Recursive logical `!`,
fixed-width integer complement `~`, and arithmetic negation `-` prefixes wrap
a completed operand from the admitted primary, group, and cast slice before
that value enters the binary frame; borrow/reference and other contextual
unary forms remain incomplete. Postfix indexing may start from a retained path
in any framed-value consumer or continue a completed literal, call, cast, or
indexed value. A completed indexed value can take up to 128 named field
suffixes. Repeated brackets retain a left-to-right expression chain. Each
bracket reuses the canonical index/range representation. Index
operands may themselves contain that same postfix form. A 128-frame context
stack suspends every live outer bracket coordinate while the inner indexed
expression is retained, then rejoins the completed inner handle as the outer
operand. This covers nested range starts and ends without copying syntax rows;
adjacent depth fails as the existing private `ExpressionDepth` resource.
Shallow struct literals
accept an exact one-member record or two-member case type path and a comma-
separated named field list with an optional trailing comma. Canonical adjacent
fields without commas, nested struct literals, and richer field expressions
remain incomplete until the shared postfix and field-expression paths admit
them. The implicit entry state and every
authored `state` own independent parameter, return-type, and contiguous mixed-
statement spans. A retained transition has either no subject or one precedence-
correct subject over path, boolean, and nonnegative decimal integer
primaries, and expands each
boolean, integer, or wildcard arm into an ordinary statement targeting one
named state. Arms also retain bare name paths and qualified two-member case
paths with optional braces. Braced case patterns own a contiguous field ledger
over shorthand, renamed and waived bindings and fixed path-expression matches, preserving the
distinction between `Type::Case` and `Type::Case {}` plus exact binding and
field spelling for later resolution. A named terminal target uses the ordinary call-argument parser. Its argument
span is contiguous even when a nested call publishes argument rows first.
Operands share precedence, grouping, casts, short-circuit expressions, ordinary
calls, references, shallow struct literals and the existing indexed paths.
An index may instead use any canonical bound-presence/inclusivity
shape (`..`, `..end`, `..=end`, `start..`, `start..end`, or `start..=end`)
over those retained bounds. It is represented by a separate range expression
rather than folded into the indexed node. Target argument lists retain an
optional trailing comma. A
subjectless block retains each condition as an ordinary expression with its own
source span. A wildcard has a distinct guard kind. Computed or multiple subjects, record/tuple patterns, renamed or
waived fields, non-path fixed values, proof selectors, richer guards, richer
target arguments,
terminal/value/self targets, continuations, and `match` remain implementation-
incomplete. Richer expressions and statements likewise stop the root as
incomplete, and no body is skipped as opaque text.
Local-data statements retain canonical `let [mutable] name: Type [= expression];`
syntax in a dedicated row. `mutable` is contextual, so `let mutable: T;` still binds an
immutable local named `mutable`. Locals share the borrow-capable type engine and the
same bounded expression reducer as assignments, including repeatable postfix
indexing with path or decimal indexes and the same canonical range shapes;
absent initializers are represented by an explicit presence bit rather than a
valid-looking handle. Ordinary call
initializers share the statement-call target, static-argument, runtime-argument,
and delimiter parser but own a distinct expression row with an explicit
optional receiver handle. Call arguments retain the existing primary slice and
reuse the same reducer.
Casts reuse the expression and type engines for assignments, locals and
both kinds of call argument. A carrier target retains its type handle.
Each explicit domain addition, removal or retention keeps its domain span. In
`value as T +D` the added-domain row owns the completed carrier expression.
The carrier span ends at `T` and the outer span ends at `D`. A bare sign
following a cast starts a domain entry. Parentheses permit arithmetic on its
result. A single `as` accounts for all its entries together. The scalar
compiler checks the input and output arithmetic policies before evaluating
the operand once. Both `-Trapping +Wrapping` and `+Wrapping -Trapping`
therefore exchange policies on a u64. A second `as` starts a separate cast.
One terminal expression statement may close a state directly. Its current
values are a self/name/member path, boolean, nonnegative decimal
integer, string, or a path followed by one or more indexes with path, decimal,
or canonical range operands, and such a primary may continue through the shared
binary-operator reducer — including a statement-leading path — with `}`
terminating the statement only when no expression group is open. The statement
points directly to the shared expression node; indexing has no assignment-only
syntax representation.

Each completed machine owns zero or one implicit entry followed by its explicit
states in source order, matching the canonical parser. Outer and nested
`requires` clauses retain shared predicate-expression rows with exact owner
slices, including repeated clauses, conjunctions and calls. The parser does not
infer predicates or resolve their meaning; the compiler's narrower checked
requirement profile is described above. Parameters, a return,
implicit statements, or an otherwise empty machine require the implicit entry;
an explicit-state-only machine without those forms does not manufacture one. A
free machine uses the generated `entry` identity, while an attached machine
names its entry with the final authored declaration-path member. Machine and
termination-witness paths share the general path-member arena. A machine
snapshots its declaration path before reading parameter types and clauses. A trailing
parameter comma rejects as malformed. Constrained slice elements and the
`Slice<T>` spelling, return types placed after clauses, other generics, remaining
machine-clause forms beyond the supported contracts, and `boundary` forms; target
declarations and public target-scoped combinations, other public roots,
other bodyless declarations, and other body forms
remain incomplete. The parser never skips a body as opaque
syntax. In the current 73-root `C` closure, all 113
machine-header parameter occurrences, all 73 root parameter lists, and all 73
root headers are representable. All 73 roots are complete: all 57 bodies that
reach parsing, plus sixteen target-scoped bodyless external leaves. The bodyful
set includes
four initial call-only roots, seven roots using the retained assignment slice,
four target-provider roots using path-only static call arguments, the string-argument
`psi` package build root, `Lexer::initialize`, the canonical Omega package build
root, `Lexer::{is_whitespace,push_decoded}` through explicit states and
transitions, and `ConsoleNativeProvider::{write,write_line}` through exact
satisfies and reach clauses, plus `Lexer::tokenize` through the same ordinary
call/assignment/transition statement grammar and `Lexer::emit_punctuation`
through retained addition. `Lexer::is_identifier_start`,
`Lexer::is_identifier_continue`, and
`Lexer::hex_digit_value` complete through precedence-aware transition subjects
and subtraction. `SourceUnit::append` and `TokenStream::push_decoded` complete
through retained indexed assignment places. `SourceUnit::byte_or_nul` completes
through the same indexed expression in terminal value position, and
`TokenStream::push` completes once its indexed place and named transition-target
argument are both retained. `Lexer::{retain_token,classify_keyword,
lex_identifier,lex_whitespace,consume_suffix}` complete through canonical local
bindings and the shared additive initializer path. `Lexer::{lex_line_comment,
lex_block_comment,copy_source_to_decoded,is_raw_string_candidate}` complete
through retained call-valued local initializers.
`Lexer::{validate_utf8,dot_starts_float,reject_raw_string_candidate}` complete
through retained additive ordinary call arguments; `Lexer::lex_next` advances
through the same syntax. Path-valued casts complete
`Lexer::{consume_digits,lex_punctuation,lex_next}` and
`Parser::load_current`. Reusing the indexed-expression builder for local
initializers completes `Lexer::span_equals`; the three Parser roots that use the
same form advance to qualified token-pattern guards. Retaining qualified case
paths, shorthand bindings, and fixed path matches then completes `Main::main`,
`Lexer::digit_in_base`, and `Parser::{parse_data,skip_trivia,parse}`. The
shared grouping and multiplicative reducer then completes
`Lexer::{decode_at,lex_cooked_string}`. Recursive logical-not retention completes
`Lexer::lex_number`. The sixteen external leaves are the four
`ConsoleNativeProvider::{read_line,read_byte,write_byte,exit_process}`
realizations for each selected target. `console_write_bytes` completes through
shared indexed transition-target arguments: `bytes[0]` owns an integer index,
while `bytes[1..]` owns an open-ended range index. Closing the current root
census does not imply that the parser implements unexercised Omega syntax or
that `D` has semantic, lowering, or emission closure.

Data syntax retains an optional `[copy]` property, repeated `requires`
predicates, bare named fields,
payload-free cases, contextual `case: Type` fields, structured case payloads
over the same bare named type leaf, one unqualified `Base in Domain`
constraint, recursively nested fixed arrays `[Type; length]`
with an unsuffixed decimal length, optional final member/case semicolons, mixed
field/case order, and relative spans in separate live-prefix tables. A single
named domain can qualify the leaf, an inner array or the outer array. Its
constrained type node wraps the carrier selected by the source brackets. A case reaches its
contiguous payload-field span in a separate arena; direct and payload fields
share one binding control path. Type references are postorder tagged nodes: a
constrained root points to its base and stores the start and end of the domain name, while
an outer Reference points backward to its complete referee tree and retains
shared/mutable/write-only access plus an optional exact lifetime span.
FixedArray and Slice nodes point backward to their element; FixedArray also
retains the exact length span. SelfType and Unit need no payload.
The constrained type node owns the entire single-name domain span. Array lengths
retain exact spans without interpreting their values. Data predicates retain source-ordered
ordinary expression roots and clause spans; each data row owns its exact
requirement span, including an empty span for a declaration with no predicates.
The common expression builder owns precedence, groups, paths, literals and
operators. Completing a predicate leaves the next `requires` or body-opening
brace to the data-header parser. These are retained syntax, not an assertion
that D already proves the data's default-domain obligations. Scalar range type
suffixes are syntax failures at their opening bracket. Bracket syntax
uses a bounded invocation-local frame stack and emits named, array, and slice
nodes in postorder, so every child index points backward.
Compact kind/index ledgers reach the use/data/machine rows and field/case child
spans instead of duplicating coordinates. Qualified, indexed, intersected,
combined, exclusive, expression-bound, or multiple constraints; constrained
slice elements, the `Slice<T>` spelling, generic type arguments, rich array elements or
lengths; numbered identities;
field relevance; other public roots; and every other unimplemented valid form
stop as implementation-incomplete rather than becoming false Omega rejections.

The provisional backing tables hold 4,096 root/use/data/machine/state rows and
16,384 path-member/data-member/direct-field/payload-field/case/machine-parameter/
machine-clause/data-requirement/type-node/statement/call-statement/call-expression/
cast-expression/assignment/local-data/transition/expression/binary-expression/
unary-expression/
indexed-expression/
argument/transition-pattern-field/static-machine-
argument/struct-literal/struct-field rows,
plus 128 scratch type-array frames and 128 expression values, binary operators,
unary prefixes, group boundaries, and nested-index contexts.
Only rows below their corresponding count may be inspected after `Complete`;
every other status may leave unowned partial prefixes and authorizes no
syntax-tree consumer. A repeated invocation invalidates old rows by resetting
every count. Root capacity dominates use/data/machine capacity, while `States`
becomes independently exhaustible as soon as one machine may own multiple
states. Data-member capacity dominates direct-field/case capacity. Fields,
parameters, and state returns share the type-node table, making `TypeNodes`
independently exhaustible. Its equal ceiling dominates payload-field and
machine-parameter capacity because each retained row owns at least one type node.
Import paths, call receivers, and path expressions share the
independently exhaustible path-member arena after each machine snapshots its
declaration path. Every retained call or transition-target argument owns one
expression row, so `Expressions` dominates the equal argument table. Every
call, assignment, local-data statement, or
transition-arm row owns one statement, so `Statements` dominates all equal
statement-variant tables. Every local owns at least one type node, so
`TypeNodes` also dominates the equal local-data table. Assignments own two
expressions, making the
expression table independently exhaustible. Every retained struct field owns
one value expression
and every struct literal owns its expression node, so `Expressions` dominates
both equal struct tables. Every static machine argument owns at least one
same-capacity path-member row, so `PathMembers` dominates that table as well.
Every retained machine clause owns at least one path member, so `PathMembers`
also dominates the equal clause ledger.
Every retained transition-pattern field owns one path member for its spelled
field, so `PathMembers` also dominates the equal pattern-field ledger even when
a fixed match adds its own expression path.
Every binary row owns its expression node, so `Expressions` dominates that
equal table as well.
Every unary row likewise owns its expression node, so `Expressions` dominates
that equal table as well.
Every indexed row likewise owns its expression node, so it introduces no new
resource distinction.
Every range row likewise owns its expression node, so `Expressions` dominates
the equal range table.
Every call-expression row likewise owns its expression node, and every retained
receiver is another expression node, so `Expressions` dominates the equal call-
expression table.
Every cast-expression row likewise owns its expression node, so `Expressions`
dominates that equal table.
Every terminal expression statement points directly to an existing expression
handle and adds no variant table; the equal statement and expression ceilings
remain independently exhaustible across the other retained forms.
The meaningful
resource distinctions are therefore `Roots`, `States`, `PathMembers`,
`DataMembers`, `TypeNodes`, `TypeDepth`, `ExpressionDepth`, `Statements`, and
`Expressions`.
These are private compiler budgets to profile against the real compiler
closure, not Omega source limits; exhaustion maps to `Incomplete` through
`outcome.epsilon`'s embedded scalar-resource projection: parser resources
occupy wire codes 3..11 under canonical-source coordinates, the request's
declared section extents occupy codes 1..2 under request coordinates, and the
Alpha tape capacities occupy codes 12..14 under emitted-artifact coordinates.

No source identity, package alias, token ledger, decoded mirror, or transferable
preflight fact is retained. The [standalone request contract](../../../wiki/spec/build/compiler_request.md) binds each relative tree to a package-owned
source unit and fixes public diagnostic/outcome framing at the outer compiler
edge. A
public validate/advance split would be false authority, while revalidating the
whole view at every token would be quadratic; neither belongs in the compiler.

### Executed nominal values and source-set construction

The earlier 818,946-byte compiler closure compiled the unchanged source
`NumericBase` declaration and `lexer_digit_in_base` machine. A 2,143-byte Omega
customer checks all 256 bytes against four independently written base predicates.
The source-constructed native Epsilon evaluator `b1cc4441…fe2e0` compiled it to
5,972 Alpha bytes (`2549c1a5…6077d7`); the executable exits 42. The same invocation
checks the parsed payload refusal, rebuilds the default nullary sum, and compiles
a namespace call across two supplied source units/packages. Removing the direct
edge refuses without a partial tape; restoring it produces the same 32-byte
Alpha tape (`9b296423…f3617d`), again exiting 42.

That invocation took 397.410 seconds with 4,972,452 KiB child peak RSS on one
shared CPU. Its 826,054-byte Epsilon source is
`c0a97e83da77dcea839d206e22b6ed209bb0a2c7eeae5ea67ef8c218f813b48b`;
request identity is
`26797a045d9cef3f50bd17410a011a45c3c623b09fdb462d2041b9027735280b`.
Its complete observation matches the independent model byte for byte:
`0aea09be26dd2ad86b37885ce3220ba4957c72cc9a10909a90c57ff28be073d2`.
This reuses the constructed evaluator; it is not a new cold construction.

The control customer at that revision covered 19 emitted executables and 37
refusal/reset checks (57.610 seconds in the model). Ten additional construction
entry observations cover source-order changes, duplicate package display names,
private and competing imports, non-transitive file imports, namespace qualification
and Unit call statements. The complete parser customer, including the distinction
between `ns::call()` and `value.call()`, passes in the model. The storage customer
also passes all 13 executables and 36 refusals with both the current qualified-byte
`SourceUnit`/`ModulePath` and its ordinary-byte counterpart. Every tape and the complete
observation are byte-identical. The ordinary-buffer fixture migration belongs with
that source change; its canary and boundary checks are unchanged.

The actual Omega compiler closure was also supplied to the production Epsilon
construction entry: 265 units / 3,698,937 source bytes, request
`d8217eeb57d3801b3a6413291013dd0bb0d6d8323ea0c77c59807202b7b8031b`.
The independent model consumed it in 144.051 seconds (978,020 KiB peak RSS)
and returned `Incomplete` at packed-source coordinate 841, unit zero relative
711: the `crashes` clause of construction `main.omg`'s `malformed` machine.
This is a measured whole-input frontier, not an Omega compiler artifact or a
self-rebuild. The existing missing effects, broader source constructs and full
public request admission remain separate compiler work.

On the Omega sources at `b95769558667`, the independent Epsilon model ran the
bootstrap compiler on 284 units / 3,822,481 source bytes, request
`aa8891b09b5edbff0cb48bddb21eb20061b5e9ee1552e19a70339799211263dc`.
Before crash support it returned `Incomplete` at the first `crashes Trap`
clause in `application/construction/main.omg`. The compiler at `0b29148fc65e` parsed
that declaration and its `crash Trap;` terminal, then returned `Incomplete`
at the `blocks` clause of `read_byte()` on line 30, packed source offset
1,034 / unit offset 904. Both runs emitted only the refusal coordinate.

With ordinary blocking-call checks, the independent model ran both that frozen
input and the 284-unit source graph at `0b29148fc65e`. The latter contains
3,822,250 source bytes, request
`94895e169fd2f36b6a2b0c93f5db3a3911432d5de63d6210bd8bd9a762387d38`.
Both runs passed the `read_byte()` header and ordinary call syntax, then
returned `Incomplete` at packed byte 2,571 / unit byte 2,441. On line 71,
`read_input()` uses `true -> block rows(...)` to enter a nested machine.
That transfer needs its own checked envelope. Both observations ended at the
refusal coordinate. The source graph has yet to reach checking of the
`ByteRead` payload and the Console and ProcessExit services.

With per-state blocking checks, the independent model ran the frozen
`b95769558667` graph above and the 284-unit graph at `2486977ecd06`
(3,822,983 source bytes, request
`8f3357f02c77d9c2700ba3343d65d62ed24cf7424a4a23e9e65615b5e1714506`).
Both runs parsed the marked `rows` target and returned `Incomplete` at
the multiplication in its third argument, `input.package_count as u64 * 3`.
The refusal is at line 71, column 65, packed byte 2,613 / unit byte 2,483.
The production entry emitted the nine-byte observation
`0002000000350a0000` on each input. Full-graph semantic checking and
production of the Omega compiler binary remain open.

The 284-unit product at `1cc98ba38f97` captures the row count before lending
the input to `rows`. Its request contains 3,823,097 source bytes, SHA-256
`828343948802ab708fc629de2156dfc933ad05bd41d65202fb8eced22e9c6528`.
With per-state blocking checks alone, the independent Epsilon model returned
`Incomplete` at the `+` in `rows(input, index + 1, count)`, line 88,
column 51. With shared terminal-call argument parsing, the same production
entry crossed that call and the rest of `read_input`. It returned
`00020000005a1e0000` at `target` in
`requester < input.package_count && target < input.package_count`,
line 209, column 55, packed byte 7,770 / unit byte 7,640.
On the original frozen `b95769558667` graph, the identical compiler and
construction entry passed the cast and multiplication in the earlier row-count
calculation. They reached the same `target` token at line 206, column 55,
packed byte 7,656 / unit byte 7,526, observation `0002000000e81d0000`.
The `2486977ecd06` graph and the frozen single-unit row-count projection
(`3c7966ee415014061465b97e08418728241b29144fa9069243f9ebb30372304e`)
reproduced the unmodified and published coordinates respectively. Each of
the four observations contains only the refusal coordinate.

With keyword-spelled transition operands, the model ran the unchanged
284-unit product from `6be46283b514` above. It completed parsing
`application/construction/main.omg` and reached
`compilation/00_scalar_operations_to_alpha.omg`. In `alpha_new_mark`,
`mark as u64 +Trapping` returned `Incomplete` at the plus, line 131,
column 21, packed byte 18,143 / unit byte 4,389. The nine-byte observation
was `0002000000df460000`.
The frozen `b95769558667` input reached the same source unit and local
coordinate, packed byte 18,029, observation `00020000006d460000`.
Neither run reached semantic checking of the entire source graph.

With carrier conversion followed by an explicit domain addition, the same
production entry passed `mark as u64 +Trapping` and the intervening emitter
machines. On the published 284-unit input it reached
`ScalarOperationKind::Copy -> copy(emitter, body, operation)` on line 577,
column 38 of `compilation/00_scalar_operations_to_alpha.omg`. The model
returned `Incomplete` at the called name, packed byte 37,331 / unit byte
23,577, observation `0002000000d3910000`. The frozen input reached the
same unit and coordinate at packed byte 37,217, observation
`000200000061910000`. Each nine-byte output contains the refusal coordinate.


The target token is lowercase `copy`, which the Omega lexer classifies as
a keyword. Line 612 declares `machine copy(...)` inside the same enclosing
machine. D's terminal target parser accepts an identifier or a boolean
result, and its nested machine name parser accepts an identifier. The
[terminal-call rule](../../../wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms)
resolves a called target as a machine and applies ordinary argument and
completion rules.

### Restored V1 borrow spelling

The V1 product at `d8a8825e3c23` uses `@mut` for Omega borrows. Its
284-unit request contains 3,823,380 source bytes, wire SHA-256
`636c924b64ae1d2a1c827466dfdcf868a89e935847e0c0f4a15b7235ba2f6405`.
The Epsilon code that implements the Omega compiler still uses Epsilon
receivers and byte views spelled `&mut self` and `&[u8]`.

At that revision, `_recycling/tests/epsilon/refinement/model.py` expects `@` in an
Epsilon receiver. It rejected both the unmodified and integrated Epsilon
compiler sources at packed byte 39,463, the receiver in
`request_and_utf8.epsilon` line 27. Both observations were
`0207279a0000`. The full-product comparison below uses the independent
model from `8092cfd9c0a5`, SHA-256
`036633fe351bd0da5e4bf8426b2ccdb87287fe423868fe686750d5135fa24490`,
on the supplied source bytes.

On that product, the unmodified compiler returned `0002000000df460000`
at the `+Trapping` in `alpha_new_mark`, packed byte 18,143. The integrated
compiler returned `0002000000d3910000` at the ordinary nested `copy`
call, packed byte 37,331 / unit byte 23,577, line 577, column 38. The
compiler and production entry together were 845,417 Epsilon bytes, SHA-256
`95ea6b8b9cd7d2e1d186c70bcda1ba1cf9821b0451f751779ea3978df196ffdd`.
Both full-product observations end with the refusal coordinate.

### Same-carrier cast comparison

The carrier-cast rule says, “The operand of a carrier cast must carry no
domain.” It defines `plain as T +D` as `(plain as T) as +D`. The bootstrap
producer below follows this rule for arithmetic policy domains as well.
[Carrier casts](../../../wiki/spec/language/domains.md#carrier-casts)

The input below selects `main`. For each row in the table, replace the body
of `mark` with the listed expression and use the listed type for both its
return and `x`.

```omega
machine mark(n:u32 in Trapping)->u32 in Trapping {(n as u32) as +Trapping}
machine main()->u8 {
    let x:u32 in Trapping=mark(3 as +Trapping);
    transition x==3 {true->pass(), _->miss()}
    machine pass()->u8 {42}
    machine miss()->u8 {1}
}
```

| Body of `mark` | Return type and `x` | Bootstrap | Rust |
| --- | --- | --- | --- |
| `(n as u32) as +Trapping` | `u32 in Trapping` | Reject | 42 |
| `n as u32 +Trapping` | `u32 in Trapping` | Reject | Reject |
| `n as u32` | `u32` | Reject | 42 |
| `(n as u32) as -Trapping` | `u32` | Reject | Reject, absent domain |
| `n as -Trapping` | `u32` | 42 | 42 |
| `(n as -Trapping) as u32 +Trapping` | `u32 in Trapping` | 42 | 42 |
| `n as +Trapping` | `u32 in Trapping` | Reject | Reject, repeated domain |

The bootstrap producer was 839,302 Epsilon bytes, SHA-256
`e30d71de073d6933f0e4a9ac3b6b530284443e50f06098ad7f4e4d29d4cfb601`.
The independent Epsilon model ran its source. Its two 306-byte Alpha tapes
ran on Linux and exited 42. All five rejected inputs emitted zero tape bytes.
The Rust-built compiler ELF from published commit
`1f4fe6db8a27f3e28f446ec277522394980f1908` was SHA-256
`9b91b6827fea60db11f69feedbbddf149a399d261b75b9e7d77cae2e7c4aa31b`.
Its native comparison entry checked that `main()` equaled 42. A second
Linux entry forwarded `main()` as the process status for the parenthesized
expansion. In the two-step Rust form, the inner cast drops `Trapping`.
Removing it again gives the diagnostic that the operand's type `u32` does
not carry `Trapping`. The combined form diagnoses the original qualified
operand and asks for explicit removal first.
The Rust lowerer in that revision accepts an arithmetic policy on an
identical scalar carrier and produces the plain target qualification.
Its `operations/lowering/src/read/casts.rs` Git blob
`dc22719ef09bb175c5e1fa7c8745984c65e926d5` also appears in
`87e11d9cecd7`. The native observations above used the earlier binary.
The same Rust ELF also compiled the retired `n as u32 in Trapping` spelling
for a plain `u32` operand and produced exit 42. The bootstrap parser returns
`Incomplete` at that spelling.

### Renamed payload bindings on the complete compiler product

The exact 304-unit construction input from `e45acdff0a9` has 3,955,617
bytes, SHA-256 `bc2b6ba207d220480187465055431f022fb5dab95040b85cd68733a99341a1ac`.
The independent Epsilon model from `8092cfd9c0a5` ran the bootstrap compiler
and its ordinary construction entry on these unchanged bytes. Before payload
renaming it returned Incomplete at byte 87,497, the `as` in
`Selection::Machine { handle as callee }`. The 871,346-byte updated guest
(SHA-256 `6ff7efc90597dc4bdda9eb9f7022f7927a4ddaa85ee3eb049eb278fa368fba88`)
returned `00020000000fc60100` at packed byte **116,239**. This is unit 6,
byte 2,047, line 48 column 18 in `scalar.omg`, on the `in` of
`[u64 in Trapping; 131072]`. The result contains the Incomplete coordinate
and no Alpha tape. [Next source construct](../../../source/subsystems/compiler/source_to_psi/pipeline/checked_trees_to_scalar_operations/scalar.omg)

The isolated binding, zero-omission and authored-order programs each emitted
Alpha tape that ran to exit 42 on the seed. The order control evaluates two
mutating calls in reverse declaration order and checks both saved values, the
omitted zero and the final counter. [Bindings](../../../tests/bootstrap/conformance/omega/payload_bindings.omg),
[omission](../../../tests/bootstrap/conformance/omega/payload_omission.omg)
and [initializer order](../../../tests/bootstrap/conformance/omega/payload_field_order.omg)

### Nested expression call controls

`python3 tests/bootstrap/independent/nested_calls.py CONSTRUCTOR NEW_DIRECTORY`
runs a supplied source-built Epsilon constructor on closed-source-set requests.
The positive cases execute the emitted tapes on the audited Alpha seed; twins
exercise signatures, lexical visibility, runtime capture, entry requirements,
blocking and trapping ceilings, and mixed ordinary/terminal cycles. The test
retains exact requests and observations. This does not claim that a development
source model alone establishes the closed-set construction route.

# Delta language

Delta is the small, typed, pure definitional language used to implement the
Epsilon compiler. A canonical Gamma-written compiler accepts this language and
emits Alpha tape directly. The older interpreter and type checker are bounded
oracles and implementation material; neither defines a second Delta language.

## Source envelope

Delta source uses the bootstrap textual-ASCII envelope. The only admitted
source bytes are HT, LF, CR, and printable ASCII. NUL, DEL, bytes above `0x7F`,
every other control byte, and a Unicode BOM reject before tokenization at their
exact byte offsets. There is no decoding, normalization, Unicode table, or
host-locale-dependent lexical rule.

Identifiers and decimal digits use explicit ASCII ranges. Exactly space, tab,
CR, and LF are whitespace. A comment ends at CR, LF, or source end. Literal
escapes, rather than raw non-ASCII source bytes, produce other byte values.

An identifier is `[A-Za-z_][A-Za-z0-9_]*`. Type and constructor names begin
with `A..Z`; function, parameter, and local names begin with `a..z` or `_`.
Keywords and the closed `bytes_*` built-ins are reserved. `;` begins a line
comment. An integer literal is an optional `-` followed by one or more ASCII
decimal digits and must fit `Int`.

## Program form

A Delta program is a sequence of algebraic-data declarations followed by typed
function declarations:

```text
program      := data-declaration* function-declaration+
data         := (data TYPE (CONSTRUCTOR TYPE*)+)
function     := (def NAME ((NAME TYPE)*) TYPE expression)
type         := Int | Bytes | TYPE
expression   := INTEGER | NAME
              | (if expression expression expression)
              | (let NAME expression expression)
              | (+ expression expression) | (- expression expression)
              | (* expression expression) | (/ expression expression)
              | (% expression expression) | (eq expression expression)
              | (lt expression expression)
              | (NAME expression*)
              | CONSTRUCTOR | (CONSTRUCTOR expression*)
              | (match expression (pattern expression)+)
              | (bytes_empty) | (bytes_single expression)
              | (bytes_length expression)
              | (bytes_get expression expression)
              | (bytes_slice expression expression expression)
              | (bytes_concat expression expression)
pattern      := NAME | CONSTRUCTOR | (CONSTRUCTOR NAME*)
```

The final source item is a declaration, not an untyped trailing expression.
Declarations are mutually visible, so forward and mutual recursion are legal.
Delta is monomorphic and fully annotated. It has no closures, higher-order
functions, mutation, effects, subtyping, or implicit conversion. Algebraic data
is immutable and nominal.

`Int` and `Bytes` are the only built-in data types. Every other type and
constructor is declared in source. Functions and constructors support arbitrary
arity; Gamma's register count is not a Delta language limit.

Delta has four grammar-distinguished namespaces: type names, constructor names,
function names, and local value names. Global declarations are unique within
their namespace: type declarations are unique among types, constructors are
globally unique among constructors because constructor uses are unqualified,
and functions are unique among functions. The same spelling may name a type and a
constructor, or a function and a local value, because the grammar determines
which namespace each occurrence consults. For example, `(data Token (Token
Int))` is well formed, and in `(f f)` the list head denotes the global function
while the argument atom may denote a local `f`. Delta has no function values.

Parameters, `let` binders, constructor-pattern binders, and catch-all pattern
binders inhabit the local-value namespace. No new binder may duplicate a name
in its active lexical environment. Parameters of one function are mutually
unique. A `let` initializer is checked in the outer environment; its binder is
active only in the body and may not duplicate an active parameter, `let`, or
pattern binder. Pattern binders are mutually unique, may not duplicate an
active outer local, and are active only in their match arm. A catch-all name is
an ordinary arm-local binder. Disjoint arms, branches, and sibling scopes may
reuse a spelling because their environments are never active together.
Duplicate pattern names reject; they never express an equality constraint.

Compilation first collects every global declaration and rejects the exact later
declaration of a duplicate in that namespace. It then resolves mutually visible
declaration types and checks bodies with scope-aware local-environment push/pop.
A local conflict is reported at the exact later binder. Lookup never chooses a
first or last row among competing declarations.

## Static semantics

The checker resolves every type, function, constructor, variable, and pattern
against its declaration. It checks parameter, function, and constructor arity;
operator operands; call arguments; declared result types; match scrutinees;
pattern constructors and bindings; and agreement among every match arm.

Every match over an algebraic type is exhaustive. Constructors may be covered
directly or by one final catch-all binding. Duplicate constructor arms, an arm
after a catch-all, a constructor from another type, and a missing constructor
reject the program. A checked Delta program therefore has no dynamic
"no arm matched" value.

This requirement closes a known correlated oracle defect: the temporary type
checker once omitted exhaustiveness while the interpreter fabricated `Int`
zero when no arm matched. The checker now rejects incomplete coverage and the
interpreter traps on the impossible runtime state as migration hardening, but
their former agreement still demonstrates why a differential cannot establish
a rule both sides omit. The direct compiler remains responsible for the
authoritative static judgment.

## Evaluation

Evaluation is pure, strict, and left-to-right. `if` evaluates only its selected
branch. `match` evaluates its scrutinee once and then its selected arm. A
function call evaluates each argument once from left to right before entering
the callee. Proper tail calls are required: terminating tail recursion cannot
also depend on an implementation return-stack ceiling.

`Int` is a checked signed 64-bit integer. `eq` and `lt` on integers produce `0`
or `1`; `if` treats zero as false and every other integer as true.

`Bytes` is an immutable finite byte sequence, not an algebraic list and not a
raw-memory address. The six `bytes_*` forms above are closed built-ins:
`bytes_empty` and `bytes_single` construct; `bytes_length` returns `Int`;
`bytes_get` returns the selected byte as `Int`; `bytes_slice` takes start and
length; and `bytes_concat` joins two sequences. Every valid `Bytes` has an exact
logical length representable as a nonnegative `Int`. `bytes_empty`,
`bytes_single`, and `bytes_slice` preserve that invariant. `bytes_concat` loads
the operands' logical lengths and traps before allocation when their exact
mathematical sum exceeds `INT64_MAX`; otherwise its result stores that exact
sum. `bytes_length` is therefore total over every valid `Bytes`. The compiler
may represent sealed input as a flat view and constructed output as chunks or a
rope, but representation and storage coordinates are never Delta values.

The authored runtime trap conditions are closed:

- the mathematical result of signed addition, subtraction, or multiplication
  is not representable as `Int`;
- integer division or remainder has a zero divisor, or applies the signed
  overflow pair `INT64_MIN` and `-1`;
- `bytes_single` receives a value outside `0..255`;
- `bytes_get` receives a negative or out-of-range index;
- `bytes_slice` receives a negative start or length, or a range not contained
  in its input; or
- `bytes_concat` would produce a logical length greater than `INT64_MAX`.

Out-of-range integer literals are static rejection rather than runtime traps.
A malformed private `Bytes` descriptor, an impossible checked state, or replay
disagreement is `InternalFailure`; physical heap, stack, input, or output
exhaustion is `Incomplete`. Neither condition is a Delta trap.

The compact primitive is required by the compiler customer. Representing the
4 MiB input profile as one `Cons(Int, Bytes)` node per byte would require at
least 64 MiB at the current 16-byte row size, while the existing Delta oracle
has a 16 MiB heap. That mismatch is structural, not an optimization problem.

Divergence remains divergence. Fuel is never Delta meaning. An evaluator may
bound work for a diagnostic run, but fuel exhaustion is an implementation
profile's `Incomplete` result and proves neither rejection nor divergence.

## Compiler-application profile

Delta itself has no byte-I/O operation. Its source semantics ends at a pure
returned value; a compiler-generated Alpha adapter may join that value to
sealed input and an external observation contract. D19 fixes that adapter
choice as one closed, sealed application-profile ID supplied alongside the
exact Delta source. The ID is part of compilation identity and reconstruction
evidence. It is not Delta syntax, an ambient host flag, a filename convention,
or a property inferred from source names.

The canonical version-1 request is one exact length-delimited byte sequence:

```text
0..7    [44 43 52 45 51 01 00 00]  (`DCREQ`, version 1, reserved)
8..11   application-profile ID, little-endian u32
12..15  Delta-source byte length, little-endian u32
16..    exact Delta-source bytes; exact end of request
```

The consuming compiler artifact's embedded metadata owns the profile-ID set.
Version 1 assigns `1` to `ConformanceBytesV1` and `2` to
`EpsilonCompilerV1`; zero and every ID unknown to that artifact reject. A later
ID does not require a new envelope version, while a representation change does.
The exact request and selected embedded metadata participate in compilation
identity. Profile facts are never repeated as request claims or inferred from
source, filenames, or ambient invocation state.

Each profile declares one exact maximum sealed-input extent satisfying
`0 <= maximum <= INT64_MAX`. Both version-1 profiles select 4,194,304 bytes.
`ConformanceBytesV1` also selects a 4,194,304-byte maximum successful output;
`EpsilonCompilerV1` selects AlphaBootstrapV2's 1,048,572-byte raw-tape maximum.
The compiler validates those facts before adapter emission; an admitted input
can therefore always become a valid Delta `Bytes`. An input or output exceeding
the selected maximum is profile-owned `Incomplete`, not a Delta trap. These
application limits are distinct from the Gamma-written compiler's own 4-MiB
Delta-source resource even where their numeric values coincide.

The two profiles are:

- `ConformanceBytesV1` requires `main : Bytes -> Bytes`. Its adapter reads
  one sealed input, invokes `main`, preflights the complete returned value, and
  publishes exactly those bytes on success. Its runtime-containment outcomes
  are profile-owned generated-program observations, not `DCOUT` or `ECOUT`.
- `EpsilonCompilerV1` requires the Delta-written Epsilon compiler's exact entry and
  result schema:

```text
(data EpsilonCompileOutcome
  (Complete Bytes)
  (Reject EpsilonRejectReason Int)
  (StorageIncompleteAt Int Int Int)
  (StorageIncompleteTotal Int Int))

(def main ((source Bytes)) EpsilonCompileOutcome ...)
```

The `Int` in `Reject` is the source byte offset. The storage-refusal payloads
are respectively `(limit, requested, source_offset)` and `(limit, requested)`;
they report only D31/D34's selected application-static-storage capacity. D17 and
`source/epsilon/LANGUAGE.md` own the source-declared closed
`EpsilonRejectReason` and `EpsilonCompileOutcome` sums. `Int` and `Bytes` remain
Delta's only built-in types; the profile injects no hidden nominal declaration.
The selected `EpsilonCompilerV1` profile owns the explicit `ECOUT`
constructor-to-wire-code table and its version. That deliberate coupling keeps
the published wire boundary stable; codes never derive from declaration order.

Before emission, the compiler resolves and retains the exact `main`, outcome
type, outcome constructors, and reason constructors. It requires exactly the
four displayed outcome cases and payloads. The `ECOUT` table must be a total
checked bijection over the complete reason sum: every exact constructor has one
unique in-range code, and every table entry identifies one exact constructor.
Matching names or shapes never select the profile and do not make nominal types
interchangeable. A schema, entry, or table mismatch is a `DCOUT` compilation
rejection; adding or removing a reason requires an explicit D17/profile-version
decision.

Only the generated `EpsilonCompilerV1` Alpha adapter performs I/O. It reads sealed
stdin, constructs the input `Bytes`, invokes `main`, and maps a returned value
as follows:

- `Complete(bytes)` validates and writes the exact raw artifact, then halts 0.
- `Reject(reason, offset)` validates the exact reason and
  `0 <= offset <= input length`, writes the accepted-edge diagnostic frame, and
  halts 1.
- `StorageIncompleteAt(limit, requested, offset)` validates the selected
  application-static-storage limit in `0..INT64_MAX-1`, `requested > limit`,
  and an in-range source offset, writes
  `ECOUT::Incomplete(ApplicationStaticStorageBytes)` in
  coordinate space 1, and halts 2. Any failed payload check is
  `InternalFailure(InvalidReturnedOutcome)`.
- `StorageIncompleteTotal(limit, requested)` performs the same limit checks,
  writes that resource in coordinate space 0, and halts 2.
- private source, heap, stack, output, or adapter exhaustion writes an
  `Incomplete` frame and halts 2.
- a Delta trap, impossible checked state, adapter contradiction, or replay
  disagreement writes an `InternalFailure` frame and halts 3.

The two storage constructors are the sole source-authored path to
`Incomplete`; input, stack, heap, output, and every `InternalFailure` remain
adapter-owned. For these constructors only, D34 defines `requested` as
`min(exact_demand, INT64_MAX)`: exact while representable, otherwise the
canonical exceeded-demand witness. The V1 frame and its zero-reserved bytes do
not change. No failure path publishes partial artifact bytes.

### Conformance observation profile

`ConformanceBytesV1` writes no byte until the complete returned `Bytes` has
passed descriptor, logical-length, traversal, and output-extent preflight. Halt
0 publishes exactly that value. Every recognized failure publishes empty
stdout. The generated-program status block is:

```text
132  Alpha VM illegal-instruction trap
248  InternalFailure
249  AuthoredTrap
250  StackExhausted
251  MemoryContainmentViolation
252  HeapExhausted
253  InputExtent
254  OutputExtent
255  unassigned and noncanonical
```

Status 132 is the Alpha VM refusing an illegal instruction, not a Delta
language trap. Status 249 is a deliberate generated-code observation of one of
Delta's closed authored trap conditions. Status 255 remains unavailable so a
shell or harness projection of `-1` cannot imitate an admitted internal
failure. Divergence produces no terminal observation. The temporary
`interp.gamma` oracle predates this block and retains private statuses interpreted
only by its own harness; they are not generated-program authority.

## Compiler boundary family

Canonical compiler edges share the boundary discipline settled for the Gamma
compiler, not Gamma's exact `GCOUT` identity:

- halt values `0..3` mean Complete, Reject, Incomplete, and InternalFailure;
- success stdout is the raw runnable tape with no wrapper;
- failure stdout is one canonical 40-byte, `0xFF`-prefixed frame whose tag agrees
  with the halt value; and
- unknown, malformed, noncanonical, or mismatched frames reject.

Each accepted-language edge owns its magic, version, reason/resource/internal
tables, and coordinate vocabulary. `GCOUT` remains Gamma-specific; the Delta and
Epsilon compiler edges use their own identities (`DCOUT` and `ECOUT`). One
parameterized decoder may validate all profiles, but no profile may interpret
another profile's frame. `DCOUT` V1 is
`[FF 44 43 4F 55 54 01 00]`; `ECOUT` V1 is
`[FF 44 43 4F 55 54 01 00]`. Their coordinate spaces are:

```text
DCOUT  0 none, 1 Delta source, 2 emitted payload, 3 internal row, 4 DCREQ
ECOUT  0 none, 1 Epsilon source, 2 emitted payload, 3 internal row
```

`DCREQ` validation precedes Delta lexing, declaration/type/match checking,
selected-profile schema validation, and lowering/emission. The fixed header
and magic/version/reserved bytes precede profile selection; profile selection
precedes the declared source-length provision; and only an admitted length is
followed by exactly that many body bytes plus one exact-end probe. Consequently
a four-byte length cannot require attacker-selected input consumption before
`Incomplete(source_bytes)`. Unknown profile and source-length exhaustion anchor
at request bytes 8 and 12 respectively. Body truncation and one trailing byte
are `malformed_request` at the first missing or trailing request byte.

After an otherwise valid frontend pass, profile-schema categories are ordered
`missing_entry`, `entry_schema_mismatch`, then `profile_schema_mismatch`.
Within one category an absent required declaration with coordinate space
`none` precedes located defects, then located defects use their earliest Delta-
source coordinate. Missing `main` has no coordinate; a wrong present `main`
anchors at its declaration name. `EpsilonCompilerV1` uses
`profile_schema_mismatch` with no coordinate for an absent required nominal
member and with a source coordinate for a present malformed member or reason-
code bijection. `ConformanceBytesV1` cannot emit that reason.

The closed tables are `compiler/dcout-v1.tsv`,
`compiler/ecout-v1.tsv`, `compiler/profiles-v1.tsv`, and
`compiler/conformance-observations-v1.tsv`. They are checked projections of
constants embedded in the compiler artifact, not files consulted by the
completed offline runtime. Generated-program statuses 248 through 254 remain
separate runtime observations, never compiler-boundary cases.

`dcout-v1.tsv` also records the request-profile contexts in which each code is
canonical. `unselected` means no valid profile has yet been established. The
request/outcome join checks this column because a detached DCOUT frame does not
repeat the profile ID; a profile-impossible code is a noncanonical compiler
result rather than an authored rejection.

## Compilation requirements

The Gamma-written Delta compiler type-checks before emission, erases types into a
defined runtime representation, and emits Alpha tape directly. Its private
frame ABI must support arbitrary function and constructor arity and preserve
proper tail calls. It may not publish an interpreter plus serialized syntax,
invoke an external evaluator, add Delta operations to Gamma or Alpha, or make a
private capacity into Delta semantics.

`compiler/delta_compiler.gamma` now owns the retained static-semantics frontend;
`interp.gamma` remains a semantic oracle. The oracle's output convention--
printing a value while also placing an integer projection in the halt word--is
not the compiler boundary. The interpreter is absorbed only where an isolated
algorithm is economical and otherwise reduced or deleted once the direct edge
subsumes its diagnostic role. The Python reference is temporary differential
scaffolding and never part of the completed offline bootstrap closure.

The interpreter traps immediately on an unmatched pattern as a temporary
hardening measure. The canonical compiler must instead reject the
nonexhaustive source statically.

## Current oracle coverage

The current oracle gates check the outer source contract before parsing: the
evaluation surface passes 48 focused cases, the compiler frontend passes 82
plus one exact emitter-substrate probe, six executed runtime-containment
probes, 16 checked-`Int` paths, 31 source-to-code lowering cases, resolved-call
and resolved-constructor bridge payloads, four byte-determinism comparisons,
14 compact-`Bytes` runtime paths, two executed
arbitrary-arity/frame-ABI paths, three algebraic-value ABI paths, eight
sealed-input runtime paths, and one sealed-input reconstruction comparison; the
temporary independent evaluator agrees on 106 fixed or generated cases.
These counts include CR-terminated comments and fail-closed NUL, vertical-tab,
DEL, and high-byte controls. They cover bounded parts of this contract but do
not constitute the missing compiler edge or establish an obligation both
oracles omit.

The former Delta proof-kernel copies and the old generic canonical-byte and
terminal-codec prototype were not consumed by a live artifact admission and are
retired to Git history. Future artifact-specific decoding belongs beside the
artifact it admits.

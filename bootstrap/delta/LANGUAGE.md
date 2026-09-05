# Delta language

Delta is the small, typed, pure definitional language used to implement the
Epsilon compiler. Its selected Gamma-authored implementation edge is currently
open. The downgraded concatenative compiler and older bounded oracles are
implementation evidence; none defines a second Delta language.

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
              | (let NAME TYPE expression expression)
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
              | (bytes_concat expression expression)
pattern      := CONSTRUCTOR | (CONSTRUCTOR NAME*)
```

The final source item is a declaration, not an untyped trailing expression.
Declarations are mutually visible, so forward and mutual recursion are legal.
Delta is monomorphic and fully annotated, including every `let` binder. It has
no closures, higher-order
functions, mutation, effects, subtyping, or implicit conversion. Algebraic data
is immutable and nominal.

`Int` and `Bytes` are the only built-in data types. Every other type and
constructor is declared in source. Functions and constructors support arbitrary
arity; Alpha's register count is not a Delta language limit.

Delta has four grammar-distinguished namespaces: type names, constructor names,
function names, and local value names. Global declarations are unique within
their namespace: type declarations are unique among types, constructors are
globally unique among constructors because constructor uses are unqualified,
and functions are unique among functions. The same spelling may name a type and a
constructor, or a function and a local value, because the grammar determines
which namespace each occurrence consults. For example, `(data Token (Token
Int))` is well formed, and in `(f f)` the list head denotes the global function
while the argument atom may denote a local `f`. Delta has no function values.

Parameters, `let` binders, and constructor-pattern binders inhabit the
local-value namespace. No new binder may duplicate a name
in its active lexical environment. Parameters of one function are mutually
unique. A `let` initializer is checked against the binder's declared type in
the outer environment; its binder is active only in the body and may not
duplicate an active parameter, `let`, or
pattern binder. Pattern binders are mutually unique, may not duplicate an
active outer local, and are active only in their match arm. Disjoint arms,
branches, and sibling scopes may
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

Every match over an algebraic type is exhaustive. Duplicate constructor arms,
a constructor from another type, and a missing constructor
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
raw-memory address. The five `bytes_*` forms above are closed built-ins:
`bytes_empty` and `bytes_single` construct; `bytes_length` returns `Int`;
`bytes_get` returns the selected byte as `Int`; and `bytes_concat` joins two
sequences. Every valid `Bytes` has an exact
logical length representable as a nonnegative `Int`. `bytes_empty`,
`bytes_single` preserves that invariant. `bytes_concat` loads
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
Version 1 assigns `1` to `ConformanceBytesV1`; zero and every other ID reject.
A later ID does not require a new envelope version, while a representation
change does. The exact request and selected embedded metadata participate in
compilation identity. Profile facts are never repeated as request claims or
inferred from source, filenames, or ambient invocation state.

`ConformanceBytesV1` selects exact 4,194,304-byte maximum sealed-input and
successful-output extents. It requires `main : Bytes -> Bytes`. Its adapter
reads one sealed input, invokes `main`, preflights the complete returned value,
and publishes exactly those bytes on success. An input or output exceeding the
selected maximum is a profile-owned generated-program observation, not a Delta
trap or compiler-boundary result. These limits are distinct from the
implementing Gamma evaluator's source and sealed-input resources even where
numeric values coincide.

The former profile ID 2 and `EpsilonCompilerV1` schema are retired. The
Delta-written Epsilon implementation executes Epsilon and does not request an
Epsilon-to-Alpha adapter from this compiler.

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
failure. Divergence produces no terminal observation.

## Compiler boundary family

Canonical compiler edges share one boundary discipline:

- halt values `0..3` mean Complete, Reject, Incomplete, and InternalFailure;
- success stdout is the raw runnable tape with no wrapper;
- failure stdout is one canonical 40-byte, `0xFF`-prefixed frame whose tag agrees
  with the halt value; and
- unknown, malformed, noncanonical, or mismatched frames reject.

Each accepted-language compiler edge owns its magic, version,
reason/resource/internal tables, and coordinate vocabulary. The Delta compiler
uses `DCOUT` V1, `[FF 44 43 4F 55 54 01 00]`, with coordinate spaces:

```text
0 none, 1 Delta source, 2 emitted payload, 3 internal row, 4 DCREQ
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

After an otherwise valid frontend pass, an absent `main` has no coordinate and
a wrong present `main : Bytes -> Bytes` anchors its schema rejection at the
declaration name.

The future compiler artifact embeds its closed tables. No host table is a
runtime input or semantic authority. Generated-program statuses 248 through
254 remain separate runtime observations, never compiler-boundary cases.

## Compilation requirements

The Gamma-authored Delta compiler must type-check before emission, erase types
into a defined runtime representation, and emit canonical Gamma source. The
selected Beta-authored Gamma evaluator executes that receipt under the required
application profile; Beta alone encodes Alpha. Generated Gamma must support
arbitrary function and constructor arity and preserve proper tail calls. The
Delta compiler may not invoke an external evaluator, add Delta operations to
Gamma or Alpha, interleave direct Alpha emission with Delta checking, or make a
private capacity into Delta semantics.

The selected compiler source has a staged implementation for arbitrary-field
finite ADTs, including recursive data, plus exhaustive matches, exact global
call signatures, lexical local-scope resolution, and the complete
scalar/nominal type relation. Authored signed arithmetic has checked runtime
lowering, ordinary tail calls survive `if`, `let`, and `match`, and the five
normative `Bytes` operations lower through a private length-bearing rope.
`ConformanceBytesV1` framing, schema validation, and generated execution are
implemented. Strict DCREQ admission publishes canonical DCOUT request
rejections and source-length refusals. Source-envelope, lexical token and
integer-range, structural syntax, duplicate-global, and post-frontend
entry-schema diagnostics also publish canonical DCOUT. Declaration and body
checking publish codes 9 through 18 for local/pattern conflicts, unknown names
and types, type and arity disagreement, duplicate match cases, and incomplete
match coverage. The complete global
census precedes this phase. It accounts for D30's 32,768 authored function
rows, checking an exact duplicate before provisioning each fresh row. A fresh
32,769th function returns `Incomplete` resource code 4 at its name-token start,
limit 32,768 and requested 32,769, before insertion or declaration-type
resolution. Complete grammar/depth checking precedes this count. Generated
helpers and typed metadata copies do not add authored rows; Gamma's separate
4,096-function executable-program bound remains a later obligation.
The same census accounts for 65,536 authored constructor rows globally across
data declarations. Each fresh constructor advances that count once; payload
fields and resolved metadata copies do not consume additional constructor rows.
Duplicate constructor lookup precedes provision. A fresh 65,537th constructor
returns `Incomplete` resource code 3 at its name-token start, with limit 65,536
and requested 65,537, before its metadata or trie row is allocated. Duplicate
types reject before any constructor in that declaration is provisioned.
The type-row total includes the two builtins, `Int` and `Bytes`, plus each
fresh nominal declaration. The selected 65,536-row limit therefore admits
65,534 nominal declarations. Annotation occurrences and metadata copies do not
add type rows. Duplicate type lookup precedes provision; a fresh declaration
requesting total row 65,537 returns `Incomplete` resource code 2 at its
type-name start, with limit 65,536 and requested 65,537, before type metadata
or any constructor processing. Builtin identities need no nominal trie entries,
but that representation choice does not increase the selected logical total.
Local environments account for D30's 65,536 active binding rows. Each function
starts with an empty environment; its parameters, active `let` bindings, and
current pattern bindings share this limit. A fresh 65,537th binding returns
`Incomplete` resource code 5 at its name-token start, with limit 65,536 and
requested 65,537, before insertion. Parameter conflict and annotation checks
precede provision. A `let` resolves its annotation, checks conflict, provisions
its body binding, then checks its initializer against the unchanged outer
environment. Pattern outer conflicts and repeated-binder checks precede each
provision. Saved environments restore their counts with their names, so sibling
scopes and disjoint arms do not accumulate bindings. Retained snapshots and
trie nodes do not add active rows; this is not a generated-runtime slot limit.
Declarations, constructors, and fields are visited
in authored order; each parameter's conflict check precedes its own annotation,
parameters precede the result type, and all declarations precede all bodies.
Retained balanced syntax and grammar judgments feed the global census,
signature resolution, complete body typing, and selected-profile validation.
Lowering then completes an expanded Gamma program before the first output byte.
Explicit continuations retain pending children, while the plan records binding
identities and generated expression-list heights. A separate normalizer reuses
fitting subtrees and extracts over-height fragments into generated functions
under a 255-list body budget. Captures pass established values through fresh
parameter identities, and replacement calls retain the fragments' evaluation
and tail positions. A serializer prints the resulting plan rather than
selecting lowering rules during publication; exact spans supply admitted names
and literal bytes.
Serialization first counts the complete payload without writing, including
fixed helpers, profile text, definition separators, and the entry-owned final
LF. Expression nodes cache exact occurrence extents using the serializer's
shared atom and prefix formatting, so preflight need not unfold shared children
again. Rebuilt nodes refresh that summary; extent addition is checked, not
saturated. A count above 16,777,212 returns `Incomplete` code 12 in emitted-payload
coordinate space 2, at byte 16,777,212, with limit 16,777,212 and the exact
complete requested count. Count and publication share formatting ownership;
no partial artifact precedes this refusal.
Grammar implements D30's 1,024-level expression `parse_depth` profile: bodies
start at level 1, expression children including atoms advance by one, and match
arm bodies are at their enclosing match's level plus one. Declaration, parameter,
and pattern structure adds no expression levels. Before judging a level-1,025
expression, it returns `Incomplete` code 8 at that node's source start, with
limit 1,024 and requested 1,025. Complete balanced parsing precedes this check.
The syntax producer separately provisions D30's 114,294,752 syntax-arena bytes.
Its source-owned ledger counts parser nodes, both construction spines, parser
frames, the program root, and grammar work at the selected evaluator's actual
40 bytes per immutable pair. Usage is cumulative across completed lists and
declarations; phase-outcome carriers and later compiler phases are excluded.
Each allocation group is checked before allocation. Refusal returns
`Incomplete` code 7 at the owning source construct (or EOF for the final program
spine), the selected limit, and the exact requested cumulative bytes. Allocation
granularity can leave unusable tail bytes without changing the limit. Earlier
request, byte, and lexical failures retain precedence, and a reached syntax
refusal stops before any later parse or grammar judgment.
Stack-safe compiler traversal does not guarantee generated Gamma admission
throughout that depth. The selected evaluator separately bounds each generated
body at 255 nested expression lists; normalization handles that nesting bound,
but generated helper count, non-tail runtime contexts, and immutable storage
remain separately bounded. Full generated-profile admission and other
compiler-owned resource/internal DCOUT outcomes remain open; underlying
evaluator failures do not substitute for those
outcomes. These frontend judgments do not close the Delta edge or establish
full resource conformance. The complete compiler artifact remains
absent. The former concatenative-Gamma
implementation is retained only under Delta-owned bootstrap material and does
not define a second route.

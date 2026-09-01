# Delta v1 language contract

Delta is the closed compiler-host language used to author the first full Omega
compiler, `D`. Its spelling is deliberately familiar to Omega authors, but its
meaning is independent and completely specified here. An Omega document, a
compiler implementation, a historical source file, or an accepted program
cannot add to or amend this contract.

Delta v1 is intentionally small. It has finite nominal data, fixed storage,
checked scalar operations, deterministic state-machine control, and one sealed
byte-I/O capability. It has no packages, heap, proof language, dependent types,
or implicit host services.

## 1. Source and judgments

A Delta source closure is presented to the compiler as one finite byte string.
Package resolution and closure membership happen before Delta checking. The
closure owner records every member by stable source identity, byte length, and
SHA-256 digest, then packs the members in stable source-identity order. Member
paths are diagnostic locations only; they are not Delta names or namespaces.

Delta has two judgments:

```text
CheckDelta(source)
  -> Accepted(program)
   | Reject(reason, packed_offset)

RunDelta(program, stdin)
  -> Exit(code, stdout)
   | Trap(kind, stdout_prefix)
   | Diverges(stdout_prefixes)
```

`Reject` is a checking result and occurs before execution. `Exit`, `Trap`, and
`Diverges` are execution results. `stdout` and `stdout_prefix` are exact byte
sequences; `stdout_prefixes` is the ordered finite-prefix trace of a divergent
execution. Divergence means an actual infinite execution, not that a bounded
evaluator ran out of steps.

`Incomplete` and `InternalFailure` are outcomes of a compiler, checker, or
bounded evaluator, not Delta program results. They grant no Delta judgment and
publish no partial artifact. Delta v1 has no implicit allocating operation and
therefore no language-level `Exhausted` result.

## 2. Source bytes and tokens

Delta source obeys the lattice-wide closed textual-ASCII envelope:

```text
HT (0x09), LF (0x0A), CR (0x0D), and printable ASCII (0x20..0x7E)
```

Every other byte rejects before tokenization at its exact packed byte offset.
There is no source decoding, Unicode normalization, locale-sensitive
classification, or newline rewriting.

Space, tab, CR, and LF are whitespace. A `//` comment continues through the
next CR, LF, or source end. Identifiers match
`[A-Za-z_][A-Za-z0-9_]*`. Decimal digits are exactly `0` through `9`.

Character literals denote one byte. Their direct content is one printable
ASCII byte other than quote or backslash. String literals denote immutable byte
sequences and admit printable ASCII other than quote and backslash. Both forms
use this closed escape set:

```text
\n  \r  \t  \"  \\  \xHH
```

`HH` is exactly two hexadecimal ASCII digits. Unknown escapes reject.
Unterminated character or string literals report their opening quote.

The universally reserved words are:

```text
boundary trait data case machine state transition
let return assert true false self mut i32 u8 never
```

There are no contextual keywords. In particular, `use`, `in`, `requires`,
`ensures`, `terminates`, `by`, `min`, and `max` are ordinary identifiers.

## 3. Grammar

The notation below uses `?`, `*`, and `+` conventionally. Punctuation is
literal. A machine with no return annotation is resultless.

```text
program        := declaration+
declaration    := boundary_decl | data_decl | machine_decl

boundary_decl  := "boundary" "trait" IDENT "{" boundary_machine* "}"
boundary_machine
               := "machine" IDENT "(" parameters? ")" return_type? ";"

data_decl      := "data" IDENT "{" data_member* "}"
data_member    := IDENT ":" type ";"
                | "case" IDENT payload? ";"
payload        := "(" parameters? ")"

machine_decl   := "machine" IDENT "(" parameters? ")"
                  return_type? machine_body
                | "machine" IDENT "::" IDENT "(" receiver
                  ("," parameters)? ")"
                  return_type? machine_body
receiver       := "&" "mut" "self"
parameters     := parameter ("," parameter)*
parameter      := IDENT ":" type
return_type    := "->" type

type           := "i32" | "u8" | "never" | IDENT
                | "[" type ";" NAT "]"
                | "&" "[" type "]"

machine_body   := "{" statement* terminal? state_decl* "}"
state_decl     := "state" IDENT "(" parameters? ")" state_body
state_body     := "{" statement* terminal? "}"

statement      := "let" IDENT ":" type "=" expression ";"
                | place "=" expression ";"
                | call ";"
                | "assert" expression ";"

terminal       := transition
                | "return" expression? ";"

transition     := "transition" expression "{" transition_body "}"
transition_body
               := nonwildcard_arm+ wildcard_arm? | wildcard_arm
nonwildcard_arm
               := nonwildcard_pattern "->" continuation
wildcard_arm   := "_" "->" continuation
nonwildcard_pattern
               := INT | "true" | "false"
                | IDENT "::" IDENT binder?
binder         := "{" (IDENT ("," IDENT)*)? "}"
continuation   := postfix_expression
                | "return" expression?

place          := postfix_expression
call           := postfix_expression
arguments      := expression ("," expression)*

expression     := logical_or
logical_or     := logical_and ("||" logical_and)*
logical_and    := bit_or ("&&" bit_or)*
bit_or         := bit_xor ("|" bit_xor)*
bit_xor        := bit_and ("^" bit_and)*
bit_and        := equality ("&" equality)*
equality       := comparison (("==" | "!=") comparison)*
comparison     := shift (("<" | "<=" | ">" | ">=") shift)*
shift          := additive (("<<" | ">>") additive)*
additive       := multiplicative (("+" | "-") multiplicative)*
multiplicative := unary (("*" | "/" | "%") unary)*
unary          := "-" unary | postfix_expression
postfix_expression
               := primary postfix_suffix*
postfix_suffix := "." IDENT
                | "(" arguments? ")"
                | "[" expression "]"
                | "[" expression? ".." expression? "]"
primary        := INT | CHAR | STRING | "true" | "false" | "self" | IDENT
                | IDENT "::" IDENT ("(" arguments? ")")?
                | "(" expression ")"
```

`NAT` is a nonempty decimal integer with no sign. `INT` is a nonempty decimal
integer token; unary `-` is separate. The one magnitude `2147483648` is admitted
only as the operand spelling of `-2147483648`; every other integer token must be
within nonnegative `i32` range. Binary levels associate left. Postfix and unary
bind more tightly than every binary level.

Every owner-qualified data machine begins with `&mut self`; a receiverless
owner-qualified machine is not a Delta declaration. An unqualified declaration
accepts ordinary parameters only, so `&` in its input list is
`UnexpectedToken` at that byte. Conversely, `)` or an ordinary parameter where
a qualified declaration requires its receiver is `UnexpectedToken` at that
token.

For `machine Buffer::clear(&mut self)`, the first input is exactly the mutable
`Buffer` instance. `self` is its fixed reserved binding symbol: it has the
owner's nominal type and storage place, remains active in the machine's states,
and lowers as that receiver rather than as a distinct runtime value. The parser
normalizes an authored `self` expression to the ordinary local-reference path
using the keyword's own source span. Outside a machine that introduced this
binding, `self` therefore follows the existing absent-local rule and is
`UnknownName`; there is no direct contextual-`self` rejection.

Every machine application has an authored argument list, including `()` for a
zero-parameter unqualified machine or receiver method. Owner-qualified data
machines are invoked only through a receiver postfix such as `buffer.clear()`.
For a data owner, `Owner::name` and `Owner::name(...)` are constructor syntax,
not static-machine syntax.

The checker classifies a postfix expression by its resolved declaration. A
`place` may contain only field and single-index suffixes rooted at `self`, a
parameter, or a local. A `call` must end in a resolved machine application. A
slice, `.len`, `.as_slice`, constructor, or call is not an assignable place.
Within a transition, a continuation must resolve uniquely to a state transfer,
a machine call, or a return. State names and callable names that would make
that resolution ambiguous reject. Every state transfer has an authored argument
list, including `()` for a zero-parameter state. A known bare state is not a
transfer, regardless of its arity, and is `InvalidControlTarget` at the
continuation expression's first byte. A bare known machine and an unqualified
state/machine collision produce the same judgment at that coordinate. These
are distinct source causes with one public rejection; neither arity nor the
published outcome distinguishes them.

The grammar admits no `use` declaration, attribute, domain annotation, range
type, contract clause, `terminates by` clause, special result binding,
wrapping or saturating placement, generic parameter, or package/module form.

## 4. Names, types, and closure checking

Checking is whole-closure and two-pass. After parsing, the first pass performs
one complete scoped identity census before type formation. It collects type
owners, machines, boundary members, fields or cases and their payload names,
machine and state parameters, states, `let` binders, and every syntactic
transition payload binder. Every duplicate-name failure belongs to this
declaration-collection phase. Across all scoped duplicate pairs,
`DuplicateName` reports the first byte of the globally earliest later
declaration. Collection records identities without granting visibility: a
`let` remains unavailable in its initializer, and a transition binder is
visible only in its arm's continuation.

The second pass resolves types, owners, machines, calls, states, and bodies.
Top-level declarations and states within a machine may be referenced before
their textual declaration. States are direct children of one machine body;
state bodies cannot declare nested states. Therefore packing order affects
coordinates but not valid program meaning.

Grammar position and qualification select distinct namespaces:

- boundary traits and data declarations share one type-owner namespace;
- a machine identity is `(optional owner, machine name)`, so a type owner may
  share its spelling with an unqualified machine and `parse` differs from
  `Parser::parse`;
- boundary members, record fields or sum cases, state labels, and local values
  use their exact owner-local scopes; and
- a member name may share its spelling with a local binder because `self.name`
  or another postfix `.name` is position-qualified while bare `name` is local.

Boundary members are externally realized callable identities, not source-
fillable implementation slots. Once collection identifies an authored
qualified machine's owner as a boundary trait, the declaration rejects as
`InvalidBoundary` whether or not its member name exists. Duplicate member
signatures inside the boundary declaration remain `DuplicateName`. Qualified
machine bodies on data owners are ordinary machine identities and must be
unique under that owner. `InvalidBoundary` is anchored at the first byte of the
authored qualified machine declaration and competes with every `DuplicateName`
candidate by smallest packed coordinate in the same collection phase.
Classification requires one unique owner identity. A boundary/data owner
collision contributes `DuplicateName`; qualified bodies under that ambiguous
spelling contribute no inferred owner-kind failure.

A sum case and receiver machine under one data owner may share a spelling.
`Owner::name(...)` selects the case namespace, while `value.name(...)` selects
the receiver-machine namespace; neither expected type nor arity chooses between
them. Fields likewise remain position-distinguished, and the existing
field/case member collision rule inside a data declaration is unchanged.

Delta v1 permits no active local shadowing. Machine parameters are active
throughout the invocation. A state parameter is active only in that state body.
A `let` initializer sees the current outer environment; its binder becomes
active only after successful initialization and for the remainder of that entry
or state body. A state parameter or `let` may not reuse an active machine
parameter, state parameter, or earlier local. Entry and distinct state bodies
have disjoint local environments and may reuse spellings. Locals cannot cross a
state transfer except as explicitly evaluated state arguments.

Transition payload binders are mutually unique within one arm and cannot reuse
an active machine parameter, state parameter, or earlier `let`. They are visible
only in that arm's continuation. Distinct arms are disjoint and may reuse the
same spelling; a sibling-arm reference is `UnknownName`. Every syntactic binder
participates in collection even when later checking finds an unknown case or
wrong payload arity, so an earlier-phase `DuplicateName` may be followed by
`ArityMismatch` after the duplicate is repaired. `DuplicatePattern` applies to
repeated transition selectors or cases, never to repeated binder spellings.

A `data` declaration is exactly one of:

- a record containing only named fields; or
- a sum containing only named cases with finite payload fields.

An empty declaration is explicitly a zero-field record. It has exactly one
zero-initialized value and is never an empty sum. Mixing fields and cases
rejects as `InvalidDataShape` at the declaration name. Cases may have any
finite payload arity. A sum value's home tag is its case's zero-based
declaration position; the first case is the zero-initialized home case. Foreign
integer correspondences are ordinary mapping machines, not inline case numbers
or layout facts.

Sum constructors are ordinary values. Delta v1 has no record-literal form:
records are established as declared `Main` storage, nested fields, array
elements, parameters, or returned copies, then updated through checked places.
All non-capability parameters and returns have value semantics.

Every value type is finite and nonrecursive by value. Direct or indirect
recursive records, sums, arrays, or payloads reject. Delta has no heap or
recursive reference type. Consequently, `D` represents dynamic compiler
structures in source-declared fixed arrays with `i32` indexes. For example, an
AST node may store a tag and child indexes into `[Node; N]`; `N`, the cursor,
and capacity-failure behavior are authored program state.

An array length is admitted exactly when its authored `NAT` lies in
`1..2147483647`. Zero rejects as `InvalidArrayLength` at the first byte of its
length literal. A negative spelling is not a `NAT` and therefore fails during
parsing; a positive token above `2147483647` is
`IntegerLiteralOutOfRange`. Whether the selected application profile has enough
physical storage for an admitted array is not type formation and never changes
Delta validity.

`i32` is the ordinary scalar and arithmetic type. `u8` is storage-only: it may
occur in stored data, arrays, and byte views, but arithmetic and comparisons
operate on the zero-extended `i32` read. A store to `u8` requires `0..255` and
otherwise traps as `ByteRange`.

`never` is a bottom/control type permitted only as a machine return type. It has
no values and is forbidden in fields, payloads, parameters, locals, arrays, and
views. A call returning `never` cannot be bound or assigned. A
semicolon-terminated call is syntactically a statement; when its resolved
result is `never`, it gives the block no normal return. An authored `-> never`
machine admits no normal return or falloff in any entry or state block; it may
transfer state, diverge, or terminate through another `never` call. Every
declared block is checked regardless of reachability. Any later executable
construct in that block rejects as `InvalidTerminal` at its terminating `;` or,
for a transition, its closing `}`. State declarations following the entry
sequence are not executable successors.

The permitted occurrence is the exact outer machine-return type. Every other
authored `never` occurrence rejects as `TypeMismatch` at the `never` token.

A `&[T]` is an immutable bounded view. Views may be parameters and locals and
may be passed onward. They cannot occur in stored data, sums, arrays, or return
types. There is no lifetime syntax. Static checking rejects every escaping
view. A forbidden view rejects as `EscapingView` at its outermost forbidden
`&`. That outer placement failure suppresses defects nested inside the view;
children of an admitted parameter or local view are checked normally.

Type formation constructs these placement, shape, recursion, and unknown-type
candidates against the complete declaration census, then reports the smallest
packed source coordinate independent of traversal order or wire-code number.
Its final whole-program subjudgment validates the entry shape described below;
body/control checking begins only after the complete type-formation phase
succeeds. Candidate identity is exactly `(packed offset, rejection reason)`.
Repeated derivation of one pair merges. Producing two different type-formation
reasons at one exact anchor is a compiler contradiction and therefore
`InternalFailure`, never an arbitrary reason-table tie-break. A compiler may
derive a private equality discriminator from the closed reason table, but no
separately authored kind may disagree with the reason and reason-code order
never supplies priority.

## 5. Entry and boundary

Every accepted program declares exactly this boundary trait:

```text
boundary trait Console {
    machine exit_process(return_code: i32) -> never;
    machine write_byte(value: i32);
    machine read_byte() -> i32;
    machine write_line(text: &[u8]);
}
```

No other boundary declaration is admitted in v1. `Main` is a record with
exactly one `console: Console` field and any finite program-owned fields. The
entry is exactly:

```text
machine Main::main(&mut self)
```

`Console` is a sealed entry capability, not an ordinary named value type. Its
only admitted occurrence is the exact `Main.console` field above. Any other
placement, missing field, duplicate field, or competing entry shape rejects as
`InvalidEntry` through the entry-shape judgment. `InvalidBoundary` remains
owned solely by declaration collection for authored bodies on boundary owners.

The entry-shape judgment closes inside type formation. It first asks whether an
authored owner/name candidate for `Main::main` exists, before validating that
candidate's signature. If none exists, `MissingEntry` at source extent is the
sole entry verdict; absent `Main`, `Console`, and `Main.console` components do
not add candidates. Once such a candidate exists, every malformed, duplicate,
or competing entry and supporting component is `InvalidEntry`, never
`MissingEntry`.

An authored defect anchors at the first byte of its offending entry, boundary,
member, data, field, or type construct. A required but absent supporting
component anchors `InvalidEntry` at source extent. Multiple absent components
therefore merge as the same reason/coordinate. The four `Console` members are
an unordered exact identity/signature set, and parameter binder spellings are
nonsemantic. Reordering members or renaming their binders does not change the
entry shape. No synthetic omission coordinate exists.

It has no value parameters and no return value. Normal falloff exits with code
zero. Every nonzero normal status is expressed by
`self.console.exit_process(code)`. There are no bare read, write, line, or exit
operations and no implicit sugar for the receiver-qualified calls.

The execution adapter supplies the one `Console` capability and sealed input.
`read_byte` returns the next byte as `0..255`, then returns stable `-1` at EOF.
`write_byte` appends exactly one byte and traps as `ByteRange` outside
`0..255`. `write_line` appends its exact view bytes followed by byte 10.
`exit_process` terminates after all earlier writes with the exact semantic
`i32` code.

String literals have type `&[u8]` and immutable program-lifetime backing. Their
decoded bytes may be passed directly to `write_line` or any matching parameter.
Character literals have type `i32` and denote their decoded byte.

## 6. Storage and values

Delta has no ambient heap, pointer arithmetic, filesystem, environment, clock,
network, threads, process spawning, or foreign memory. All program storage is
visible in source.

Before entry, the adapter installs `Main.console`; every program-owned `Main`
field is logically zero-initialized. Scalars and array elements are zero;
records recursively use those homes; sums select their first case with zeroed
payload storage. Each machine invocation logically creates unestablished local
slots. Physical implementations may clear those slots, but their values do not
exist until their initializers complete.

Records and sum payloads establish fields in declaration order. Constructor
arguments, call arguments, assignments, and ordinary operands evaluate exactly
once from left to right. Arrays are fixed in length and never allocate.

An array or view access evaluates its base and then its index. An index outside
`0..length-1` traps as `Bounds` before any access. A slice `a[lo..hi]` requires
`0 <= lo <= hi <= length`; otherwise it traps as `Bounds`. `.len` returns an
`i32`.

`.as_slice` is a field-like contextual postfix admitted only on a complete
fixed-array value that carries a place. It evaluates that receiver exactly once
and returns a non-place immutable `&[T]` view over the array's exact full range
`0..N`; it performs no allocation, copy, or bounds check and cannot trap. A
fixed-array value without a place contributes `InvalidPlace` at the receiver
expression's start. Every other complete receiver type, including an existing
immutable view, contributes `TypeMismatch` at the enclosing postfix
expression's start. An unresolved receiver contributes no `.as_slice`
candidate. An authored record field named `as_slice` remains an ordinary field:
contextual selection occurs only after base-type classification.

Because postfix suffixes compose, `array.as_slice()` parses as
`(array.as_slice)()` rather than as an alternate spelling of the contextual
operation. The resulting view is not callable, so the additional call suffix
contributes `TypeMismatch` under the ordinary call rule.

## 7. Scalar operations

`i32` is signed two's-complement with range
`-2147483648..2147483647`. Delta defines:

- checked `+`, `-`, and `*`, trapping as `Overflow` when the mathematical
  result is outside that range;
- `/` truncating toward zero and `%` with the dividend's sign;
- `DivisionByZero` for a zero divisor;
- `SignedDivisionOverflow` for `-2147483648 / -1` (and its remainder form);
- shifts whose count must be in `0..31`, otherwise `ShiftCount`; `>>` is
  arithmetic;
- bitwise `&`, `^`, and `|` on the 32-bit representation; and
- comparisons returning exactly `0` or `1`.

There is no implicit truth conversion. `true` is `1` and `false` is `0`.
`&&` and `||` evaluate the left operand first, require each evaluated operand
to be exactly zero or one, and short-circuit. Any other Boolean-context value
traps as `NonBoolean`. `assert` uses the same Boolean check; false traps as
`Assertion`.

`min` and `max` have no privileged meaning. Authors may declare ordinary
machines with those names.

## 8. Machines, states, and transitions

Unqualified calls bind value parameters. Every owner-qualified data-machine
call additionally binds its first input to the receiver's mutable data place;
the declaration spelling `&mut self` has exactly the owning data type. All
effects are committed before return. Recursion and state cycles are permitted;
Delta v1 does not require a termination annotation.

Callable identity resolves before arity or type checking. A data-owner
`Owner::name(...)` considers only sum cases, while `value.name(...)` considers
only receiver machines and a bare `name(...)` considers only unqualified
machines. Statement position, expected result type, argument arity, and
declaration order never select a callable namespace. A uniquely resolved
constructor is not a call statement or control target. An unqualified state and machine that both match one
transition continuation, a known bare state, and a known bare machine all
reject as `InvalidControlTarget` at that continuation expression's first byte.
These distinct causes deliberately share one judgment. A bare state retains no
state application or first-class reference: no application was authored, and
state labels are not values. A bare machine may retain the general callable
reference already used by machine resolution without becoming an application.

Body and control checking is one finite premise DAG, not traversal-driven error
recovery. Every authored child judgment is visited and derives its independent
candidate even when its parent cannot be formed. The candidate set may be
reduced online to the smallest coordinate without physically retaining later
losers. A failed child, whether it
contributes a candidate or no semantic fact, does not satisfy a parent premise
and is never replaced by an error type, guessed `i32`, place, or callable. A
parent success or rejection candidate exists exactly when every fact consumed
by that rule has resolved.

Callable resolution and argument checking branch after the callable spelling
is resolved and admitted for the current expression, statement, or control
context. Arity consumes that admitted callable and the authored argument count,
but not argument facts; argument expressions are checked independently and an
`ArityMismatch` may therefore coexist with a failure inside an argument.
Argument-type comparison additionally consumes arity success and the complete
argument facts, and only that join can produce the call result. An identity
inadmissible in the current context contributes `InvalidControlTarget` or
`TypeMismatch` without also contributing arity or argument-type failures, while
its authored argument expressions remain independently checked.

A complete expression result is exactly a value with its type and optional
place, a resultless call, or a `never` call. A place-required judgment consumes
only a complete value result. A value with no place contributes `InvalidPlace`;
an unresolved expression cannot. A resultless call used where a value is
required contributes `TypeMismatch`. When it is a direct machine or constructor
argument, that failure anchors at the authored argument expression's first byte,
including an outer grouping `(`. A `never` call is admitted only as an exact
semicolon-terminated statement or an admitted machine continuation; embedding
it in another expression contributes `InvalidTerminal`, and any later
executable construct in the same block also contributes `InvalidTerminal` at
its terminating delimiter. A grouped `never` argument retains the exact
mispositioned call-head anchor rather than the surrounding group. No mutability
discriminator exists: immutable views and other nonassignable values simply
carry no place.

Projection rules consume complete base and index/bound facts. An absent member
on any complete base contributes `UnknownName` at the member spelling. A known
member of the wrong kind, a known contextual member on an unsupported receiver,
or a non-`i32` required index or bound contributes `TypeMismatch` at the
enclosing postfix expression's start. The fixed-array `.as_slice` relation is
the one contextual exception requiring a place fact: a supported array type
without that fact contributes `InvalidPlace` rather than `TypeMismatch`.

Body/control anchors are otherwise exact. Unary, binary, call, constructor,
and postfix relational failures use the enclosing expression start; arity uses
the application start. `InvalidPlace` uses the left-side place start or the
receiver start of a place-requiring contextual postfix. A `let`,
assignment, `assert`, return, or transition value-type relation uses the
initializer, assigned value, asserted expression, returned expression, or
transition subject start respectively. A required but absent return value uses
the `return` keyword. `InvalidControlTarget` uses the continuation expression
start. `InvalidTerminal` uses the mispositioned `never` call, or the terminating
`;`/`}` of the first later executable construct after a successful `never`
statement.

All admitted body/control candidates merge by smallest packed source
coordinate. Repeated derivation of the same reason at the same coordinate is
one candidate. No two simultaneously derivable distinct reasons may share one
coordinate by construction. Finding such a pair after applying the premise DAG
is a compiler contradiction and produces outer `InternalFailure`, never a
Delta rejection chosen by reason-code order. Every relation consuming a call's
result category is blocked by that call's arity failure because the result
exists only after its arity and argument-type join succeeds. Runtime short-
circuiting, transition selection, and source traversal do not suppress static
checking of authored child expressions or arms.

Every machine entry and every declared state block has only local exit effects:

```text
Falloff | ReturnNone | ReturnValue(type) | NoNormalReturn | StateTransfer(state)
```

A resultless machine admits `Falloff`, `ReturnNone`, `NoNormalReturn`, and
`StateTransfer`. A machine returning `T` admits a structurally compatible
`ReturnValue(T)`, `NoNormalReturn`, and `StateTransfer`. A `never` machine
admits only `NoNormalReturn` and `StateTransfer`. An incompatible falloff is
`TypeMismatch` at the exact closing `}` of the entry or state body. Every block
is checked independently, including unused states; return validation performs
no reachability traversal, cycle detection, or termination proof.

State declarations are control labels inside one machine invocation. State
arguments initialize their parameters simultaneously. A transition evaluates
its subject exactly once, then inspects arms in source order. It evaluates only
the selected continuation and its arguments. Each arm has one local exit
effect. A state application is `StateTransfer` and is compatible with every
machine category because every target state is checked under that same machine
category. A resultless machine continuation becomes `Falloff` if it returns; a
`never` machine continuation is `NoNormalReturn`; and a value-returning machine
continuation is `TypeMismatch` at the continuation expression start. A value
return is written explicitly as `-> return expression`; there is no implicit
machine-tail return effect.

A transition has at least one arm. Its grammar admits at most one `_`, only as
the final arm. After parsing a wildcard arm the next token must therefore be
`}`; another pattern, including another `_`, is `UnexpectedToken` at that next
token. Wildcards do not participate in `DuplicatePattern`, whose scope remains
repeated scalar selectors and exact sum cases.

A sum transition must name every case exactly once or end with `_`; this is a
static rule and the `or` is inclusive. Naming every current case and retaining
a final `_` is legal, so adding a case does not invalidate an untouched
transition. A scalar transition may use integer or Boolean patterns and the
optional final `_`. If no scalar arm matches, execution traps as
`NonExhaustiveTransition`.

Pattern names resolve before semantic checks. An unknown owner or case is
`UnknownName` at that name. A known boundary or record owner used where a sum
case is required, a scalar selector used with a sum subject, a case used with a
scalar subject, or a case from a different nominal sum is `TypeMismatch` at the
pattern's first byte. An incompatible pattern contributes no duplicate or
payload-arity candidate.

A subject-compatible scalar selector or exact case claims its semantic identity
before payload arity is checked. A later occurrence of that identity is solely
`DuplicatePattern` at the later pattern's first byte, even when the earlier
unique case subsequently fails arity. A unique case with the wrong payload
binder count is solely `ArityMismatch` at its pattern's first byte. Only a
unique, category-compatible, arity-compatible pattern supplies complete pattern
and typed-binder facts.

Scalar selector identity is the validated `i32` value, never its token spelling:
`false`, `0`, and `00` are one selector, and `true`, `1`, and `001` are one
selector. Negative patterns are not grammatical because unary `-` is not part
of `nonwildcard_pattern`; `-1 ->` is `UnexpectedToken` at `-`. Out-of-range
positive integer tokens reject before semantic selector identity is formed.

Static sum coverage consumes a complete sum subject and completed pattern
premises. If neither every case nor a final wildcard covers the sum,
`NonexhaustiveSum` anchors at the transition subject's first byte. A failed
category, duplicate, or arity premise suppresses coverage; after that defect is
repaired, missing coverage may become the next rejection. This deliberate
two-round diagnosis is premise ordering, not traversal-dependent recovery.

The subject of a scalar transition is not a Boolean context. For example, `7`
against only `true` and `false` arms produces `NonExhaustiveTransition`, not
`NonBoolean`. `NonBoolean` applies only to `&&`, `||`, and `assert`.

## 9. Closed rejection and trap identities

`DeltaRejectReason` is this closed nominal set:

```text
InvalidSourceByte       InvalidToken          InvalidCharacterLiteral
UnterminatedString      InvalidEscape         IntegerLiteralOutOfRange
UnexpectedToken         UnexpectedEnd         DuplicateName
MissingEntry            InvalidEntry          InvalidBoundary
UnknownType             RecursiveValueType    InvalidDataShape
InvalidArrayLength      UnknownName           TypeMismatch
ArityMismatch           InvalidPlace          UseBeforeInitialization
EscapingView            InvalidControlTarget  InvalidTerminal
DuplicatePattern        NonexhaustiveSum
```

Checking phases are lexical, parse, declaration collection, type formation,
then body/control checking. The earliest packed offset within the earliest
failing phase is reported. Type-formation and body/control candidates have
separate carriers and are never compared by coordinate. Coordinates are exact:

- an invalid source byte, token, escape, name, type, place, pattern, or control
  target reports its first byte;
- an unterminated literal reports its opening quote;
- an unexpected end, missing entry, absent required entry component, or other
  whole-program omission reports the source extent;
- a duplicate reports the later declaration;
- an invalid boundary body reports the first byte of its authored qualified
  machine declaration; and
- an invalid mixed data shape reports its declaration name, a zero array length
  reports its length literal, a standalone value-position `u8` or misplaced
  `never` reports that type token, and a forbidden view reports its outermost
  forbidden `&`; and
- a body error follows the premise-DAG relation anchors in section 8 rather
  than a compiler traversal or statement-wide fallback.

The Delta compiler application's `DCOUT` v1 reject table is explicit and is
not derived from Gamma constructor order:

```text
 1 InvalidSourceByte        2 InvalidToken
 3 InvalidCharacterLiteral  4 UnterminatedString
 5 InvalidEscape            6 IntegerLiteralOutOfRange
 7 UnexpectedToken          8 UnexpectedEnd
 9 DuplicateName           10 MissingEntry
11 InvalidEntry            12 InvalidBoundary
13 UnknownType             14 RecursiveValueType
15 InvalidDataShape        16 InvalidArrayLength
17 UnknownName             18 TypeMismatch
19 ArityMismatch           20 InvalidPlace
21 UseBeforeInitialization 22 EscapingView
23 InvalidControlTarget    24 InvalidTerminal
25 DuplicatePattern        26 NonexhaustiveSum
```

Zero and unknown codes are noncanonical. Reordering the authored Gamma sum does
not change this table. D19's `DeltaCompilerV1` profile owns the table and checks
it as a bijection before adapter emission: every exact source-declared
constructor has one unique in-range code, and every row identifies one exact
constructor. Changing the closed reason set requires an explicit D17/profile
and `DCOUT` version decision.

`TrapKind` is exactly:

```text
Overflow
DivisionByZero
SignedDivisionOverflow
ShiftCount
ByteRange
Bounds
NonBoolean
Assertion
NonExhaustiveTransition
```

The Delta v1 runtime trap codes are 1 through 9 in that displayed order. This
is an explicit execution-profile table rather than declaration-order identity;
it is not the Delta compiler application's `DCOUT` reject table. A trap
preserves the exact stdout prefix written before the fault.

## 10. Resource classification

Delta distinguishes three kinds of bounds.

Source-visible semantic bounds include every array length, the fixed storage
chosen by `D`, `i32` and `u8` widths, first-case sum homes, and all authored
capacity/failure logic. Enlarging `[Node; N]` produces a different `D` source
subject, not a different Delta language.

Execution-profile bounds include sealed stdin, Alpha memory and return-stack
capacities, maximum emitted tape bytes, and a finite observation-step budget.
They are recorded with the checked compiler run but cannot change Delta
meaning.

Private implementation budgets include parser tables, syntax arenas, symbol
tables, output buffers, recursion stacks, and temporary proof or lowering
storage. Exhausting any execution-profile or private budget yields outer
`Incomplete(resource, limit, requested, coordinate?)` before publication. A
detected compiler contradiction yields outer `InternalFailure`. Neither is a
Delta rejection, trap, divergence verdict, or partial successful tape.

D31 and D34 separate valid fixed storage from one selected realization. After
`CheckDelta` succeeds, the compiler expands only the storage roots actually
reachable in the selected application. An unused large type consumes no
application storage. If one reachable expanded array occurrence alone exceeds
the selected static-storage extent, the compiler reports
`ApplicationStaticStorageBytes` with that length literal's Delta-source
coordinate. Among nested individually excessive occurrences the outermost
occurrence wins; among disjoint candidates the smallest packed source
coordinate wins. The result remains anchored at that winning occurrence's own
length literal, never at the multiplication that happened to cross a private
accumulator bound. Record-field composition, sum layout, repeated roots, and
cross-declaration totals that exceed only through composition report the same
resource with coordinate space `none`.

The selected application-static-storage limit must lie below `INT64_MAX`.
For this resource, `requested` is the canonical exceeded-demand witness
`min(exact_demand, INT64_MAX)`: it is exact when the complete mathematical
demand fits nonnegative Gamma `Int`, and is `INT64_MAX` for every larger
demand. Exact `INT64_MAX` and a larger demand are intentionally
observationally equivalent because both exceed every admissible selected
limit and produce the same coordinate and no-publication result. Thus both
attributed and aggregate forms require `requested > limit`, but D34 does not
claim that every refusal carries the arbitrary-precision total.

The Gamma implementation computes in the closed private domain
`Exact(nonnegative Int) | Overflowed`. Before adding `a + b`, it tests
`a > INT64_MAX - b`. Before multiplying `a * b`, it handles either zero factor
as exact zero, then tests `a > INT64_MAX / b` before executing the
multiplication. The zero guard precedes division and is semantic: zero-field
records can contribute zero-sized components, and a division-by-zero trap
would misclassify valid capacity analysis as `InternalFailure`. Addition with
`Overflowed`, and multiplication with `Overflowed` and no exact-zero factor,
remain `Overflowed`; a known zero factor still produces exact zero. Traversal
prefixes, trapping arithmetic, and undocumented private saturation never
define the public outcome.

Delta v1 imposes no small semantic maximum such as 128 declarations, 64
locals, four parameters, three case fields, or 1,024 states. A compiler may
have such a finite private ceiling only if it reports it fail-closed as
`Incomplete`.

## 11. Compiler application and artifact boundary

The Gamma-written Delta compiler exposes pure:

```text
(data DeltaCompileOutcome
  (Complete Bytes)
  (Reject DeltaRejectReason Int)
  (StorageIncompleteAt Int Int Int)
  (StorageIncompleteTotal Int Int))

(def main ((source Bytes)) DeltaCompileOutcome ...)
```

`StorageIncompleteAt(limit, requested, source_offset)` and
`StorageIncompleteTotal(limit, requested)` are compiler results about the
selected application profile, not Delta rejections or program results. The
former requires an in-range source offset and maps to coordinate space 1; the
latter maps to coordinate space 0. The adapter validates the exact selected
limit in `0..INT64_MAX-1` and `requested > limit`; malformed returned values are
`InternalFailure(InvalidReturnedOutcome)`. The sealed Gamma compilation request selects D19's
`DeltaCompilerV1`; source names alone select no boundary. Before emission, the
Gamma compiler requires the exact displayed source-owned nominal schema and the
complete checked reason-code bijection. A mismatch rejects through `GCOUT`
rather than producing an adapter with an unhandled runtime case. The generated
adapter owns sealed input and the `DCOUT` boundary. Halt tags are 0
Complete, 1 Reject, 2 Incomplete, and 3 InternalFailure. Complete stdout is the
unwrapped Alpha tape. Every failure uses the versioned `DCOUT` diagnostic frame
and publishes no tape bytes. A returned storage refusal and adapter resource
exhaustion both produce tag 2; traps and contradictions produce tag 3.

D30 fixes `DeltaCompilerV1` profile ID 2, a 4,194,304-byte maximum sealed
Delta input, and AlphaBootstrapV2's 1,048,572-byte maximum successful output.
`DCOUT` V1 magic is `[FF 44 43 4F 55 54 01 00]`. Its 40-byte frame uses
coordinate spaces 0 none, 1 Delta-source byte, 2 emitted-payload byte, and 3
runtime-internal row. D17 rejection codes 1 through 26 above remain unchanged.
The closed additional codes are:

```text
Incomplete
1 InputBytes  2 StackBytes  3 HeapBytes  4 OutputBytes
5 ApplicationStaticStorageBytes

InternalFailure
1 GammaTrap                    2 MemoryContainmentViolation
3 InvalidReturnedOutcome       4 MalformedBytes
5 InvalidRejectOffset          6 OutputReplayMismatch
7 AdapterContradiction         8 PublicationContradiction
```

The generated application uses the committed 15-MiB explicit Gamma stack and
112-MiB immutable heap. The exact wire table is
`source/gamma/compiler/dcout-v1.tsv`, a checked projection of constants embedded
in the Gamma-compiler artifact rather than a host runtime input. A returned
offset outside `0..input length`, a malformed private value, or an authored
Gamma trap is an adapter/internal contradiction, never a fabricated Delta
rejection. Input, stack, heap, and output exhaustion remain `Incomplete` with
their exact limit and requested extent. D34 gives application-static-storage
`requested` its bounded-witness meaning without changing the DCOUT V1 frame,
resource code, outcome constructors, or zero-reserved bytes. Application-
static-storage refusal is the only `Incomplete` class ordinary Gamma `main`
may deliberately return; adapter-private resources cannot be forged through
the source outcome.

The required compiler-correctness relation is:

```text
accepted Delta source + emitted Alpha tape
  -> the Alpha tape refines RunDelta for the selected input/resource profile
```

The checker reconstructs Delta and Alpha meaning independently. The compiler
may use private CFG, layout, or encoding representations, but may not invoke a
Beta translator, Gamma evaluator, Alpha assembler, host compiler, or other
semantic stage to finish the artifact. Agreement with another implementation
is diagnostic and never replaces checked source-to-tape refinement.

## 12. Conformance and change control

A conforming implementation provides:

1. positive and negative coverage of this grammar and every closed reason;
2. exact byte-coordinate, trap, evaluation-order, and I/O tests;
3. whole-closure forward-reference and fixed-storage controls;
4. private-budget controls proving `Incomplete` publishes no tape; and
5. direct checked Delta-source-to-Alpha-tape refinement with mutations of the
   source, input/profile, artifact, and observation.

No existing Delta corpus is authoritative. The former translator and samples
were deleted, and no compatibility behavior survives through Git history.
Future syntax or semantics enter Delta only through an explicit contract
revision justified by the concrete needs and total assurance cost of `D`.

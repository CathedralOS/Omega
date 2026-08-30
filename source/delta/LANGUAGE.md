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

machine_decl   := "machine" qualified_name "(" receiver_and_params? ")"
                  return_type? machine_body
qualified_name := IDENT | IDENT "::" IDENT
receiver_and_params
               := receiver ("," parameters)? | parameters
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
                | never_call ";"

transition     := "transition" expression "{" arm+ "}"
arm            := pattern "->" continuation
pattern        := INT | "true" | "false" | "_"
                | IDENT "::" IDENT binder?
binder         := "{" (IDENT ("," IDENT)*)? "}"
continuation   := postfix_expression
                | "return" expression?

place          := postfix_expression
call           := postfix_expression
never_call     := call
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

The checker classifies a postfix expression by its resolved declaration. A
`place` may contain only field and single-index suffixes rooted at `self`, a
parameter, or a local. A `call` must end in a resolved machine application. A
slice, `.len`, `.as_slice`, constructor, or call is not an assignable place.
Within a transition, a continuation must resolve uniquely to a state transfer,
a machine call, or a return. State names and callable names that would make
that resolution ambiguous reject.

The grammar admits no `use` declaration, attribute, domain annotation, range
type, contract clause, `terminates by` clause, special result binding,
wrapping or saturating placement, generic parameter, or package/module form.

## 4. Names, types, and closure checking

Checking is whole-closure and two-pass. The first pass collects all top-level
declarations and rejects duplicate names. The second resolves types, owners,
machines, calls, states, and bodies. Top-level declarations and states within a
machine may be referenced before their textual declaration. States are direct
children of one machine body; state bodies cannot declare nested states.
Therefore packing
order affects coordinates but not valid program meaning.

Locals remain ordered: a local enters scope at its declaration and cannot be
read until its initializer has completed. Declaration names, fields, cases,
machines on one owner, parameters, locals, and states are unique in their
respective scopes.

A `data` declaration is exactly one of:

- a record containing only named fields; or
- a sum containing only named cases with finite payload fields.

Mixing fields and cases rejects. Cases may have any finite payload arity. A sum
value's home tag is its case's zero-based declaration position; the first case
is the zero-initialized home case. Foreign integer correspondences are ordinary
mapping machines, not inline case numbers or layout facts.

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

`i32` is the ordinary scalar and arithmetic type. `u8` is storage-only: it may
occur in stored data, arrays, and byte views, but arithmetic and comparisons
operate on the zero-extended `i32` read. A store to `u8` requires `0..255` and
otherwise traps as `ByteRange`.

`never` is a bottom/control type permitted only as a machine return type. It has
no values and is forbidden in fields, payloads, parameters, locals, arrays, and
views. A call returning `never` is a terminal and cannot be bound or assigned.
An authored `-> never` machine must have no reachable normal return or falloff;
it may diverge or terminate through another `never` call. A statement after a
`never` call in the same block rejects as `InvalidTerminal`.

A `&[T]` is an immutable bounded view. Views may be parameters and locals and
may be passed onward. They cannot occur in stored data, sums, arrays, or return
types. There is no lifetime syntax. Static checking rejects every escaping
view.

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
`i32`; `.as_slice` returns the full view.

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

Calls bind value parameters and, where declared, one mutable receiver. All
effects are committed before return. Recursion and state cycles are permitted;
Delta v1 does not require a termination annotation.

A resultless machine may `return;` or fall off a reachable block. A
value-returning machine must `return expression;` on every reachable normal
path. A `never` machine has no normal return path.

State declarations are control labels inside one machine invocation. State
arguments initialize their parameters simultaneously. A transition evaluates
its subject exactly once, then inspects arms in source order. It evaluates only
the selected continuation and its arguments.

A sum transition must name every case exactly once or end with `_`; this is a
static rule. A scalar transition may use integer or Boolean patterns and at
most one final `_`. If no scalar arm matches, execution traps as
`NonExhaustiveTransition`.

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
failing phase is reported. Coordinates are exact:

- an invalid source byte, token, escape, name, type, place, pattern, or control
  target reports its first byte;
- an unterminated literal reports its opening quote;
- an unexpected end, missing entry, or whole-program omission reports the
  source extent;
- a duplicate reports the later declaration; and
- a body error reports the first token of the offending expression or
  statement.

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
not change this table. Changing the closed reason set requires an explicit
`DCOUT` version decision.

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

Delta v1 imposes no small semantic maximum such as 128 declarations, 64
locals, four parameters, three case fields, or 1,024 states. A compiler may
have such a finite private ceiling only if it reports it fail-closed as
`Incomplete`.

## 11. Compiler application and artifact boundary

The Gamma-written Delta compiler exposes pure:

```text
main : Bytes -> DeltaCompileOutcome

DeltaCompileOutcome =
    Complete(Bytes)
  | Reject(DeltaRejectReason, Int)
```

`main` can return only a complete Alpha tape or one typed Delta rejection. The
generated adapter owns sealed input and the `DCOUT` boundary. Halt tags are 0
Complete, 1 Reject, 2 Incomplete, and 3 InternalFailure. Complete stdout is the
unwrapped Alpha tape. Every failure uses the versioned `DCOUT` diagnostic frame
and publishes no tape bytes. Adapter resource exhaustion and traps produce tags
2 and 3 because pure `main` cannot return those outcomes.

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

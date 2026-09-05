# Gamma language

Gamma is the first typed functional bootstrap language above Beta. It exists to
write small source transformers that construct richer successor languages
without forcing those transformations into Beta or a low-level stack language.

## Source

Source bytes are HT, LF, CR, and printable ASCII. `;` starts a line comment.
Parentheses are punctuation and need not be surrounded by whitespace.

```text
program      := function+
function     := (def NAME (parameter*) Int expression)
parameter    := (NAME Int)
expression   := INTEGER
              | CHARACTER
              | NAME
              | (if expression expression expression)
              | (let NAME Int expression expression)
              | (OP expression expression)
              | (NAME expression*)
              | (input)
              | (read expression)
              | (write expression)
              | (pair expression expression)
              | (first expression)
              | (second expression)
OP           := + | - | * | / | % | eq | lt
CHARACTER    := printable ASCII between single quotes
              | '\n' | '\s'
```

Function declarations are mutually visible and unique. Exactly one nullary
function named `main` is required. Parameters and active `let` binders are
unique. A `let` binder is absent from its initializer and active only in its
body. Every source value and expression has type `Int`; the written `Int`
annotations are mandatory and checked structurally.

## Values and control

`Int` is one 64-bit word interpreted as signed where required. Runtime words may
also carry private immutable pair references produced only by `pair`. Decimal
integer literals accumulate modulo $2^{64}$; a leading minus computes modular
negation. Character literals produce their ASCII byte value; `\s` denotes a
space so emitted layout remains visible in source. `if` selects its second expression when its condition is nonzero
and its third expression when zero. Calls evaluate arguments from left to right.
Functions and lexical scopes isolate their bindings.

`pair` evaluates two expressions left-to-right and returns one immutable pair.
`first` and `second` project its fields and trap when applied to any word that
is not a live evaluator-created pair. Pairs may contain pairs and therefore
represent recursive trees without exposing addresses or mutation to source.
Pair references may flow only through bindings, calls, pair fields, and
projections. Conditions, integer operators, compiler effects, and `main`'s
scalar transformer result require an ordinary integer word. A pair returned by
`main` is reserved for the application-result convention below; every other
pair use at a scalar boundary traps.

`+`, `-`, and `*` have Alpha's wrapping 64-bit behavior. `/` and `%`
use Alpha's signed operations and trap for zero or `INT64_MIN / -1`. `eq` and
`lt` return zero or one. Delta owns checked arithmetic where its stronger
contract requires it.

## Compiler effects

`(input)` returns the sealed-input length. `(read index)` returns one byte and
traps unless the index lies in the sealed input. `(write value)` appends the low
byte after requiring `0 <= value < 256`, then returns the same value. These are
Gamma's only effects.

A scalar `main` appends its returned value as one final byte after requiring
that value to be in `0..255`. This lets a Gamma source transformer publish bytes
and select its final terminator explicitly.

An application source begins with the exact nullary marker declaration
`(def $application () Int 1)`. `$application` is reserved to the evaluator and
selects application failure mapping before execution. Its `main` must return the
generic result `(pair status publish)`. Both fields must be scalar, status is
`0..254`, and publish is zero or one. A status-zero result must publish; a
nonzero published result must contain at least one buffered byte; and a
discarded result must have nonzero status. The evaluator validates the complete
pair before atomically publishing or discarding every preceding `write`.
Without the marker, pair-valued `main` traps. These conventions carry no
Delta-specific type, status name, output schema, or Bytes representation.

## Boundaries

The selected Beta evaluator first censuses declarations and validates every
body, then evaluates expressions directly from source. It retains no syntax
tree and generates no code. Calls in function-tail position reuse the current
activation; a 100,000-step witness is constant-space. Non-tail calls and syntax
nesting remain explicitly bounded by the evaluator profile.

Gamma has no source-declared algebraic data, pattern matching, `Bytes` value,
higher-order functions, polymorphism, modules, ambient host access, or mutable
source-visible storage. Those features belong in Delta or later rungs.

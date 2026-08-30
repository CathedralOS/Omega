# Gamma language

Gamma is the small, typed, pure definitional language used to implement the
Delta compiler. A canonical Beta-written compiler accepts this language and
emits Alpha tape directly. The older interpreter and type checker are bounded
oracles and implementation material; neither defines a second Gamma language.

## Source envelope

Gamma source uses the bootstrap textual-ASCII envelope. The only admitted
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

A Gamma program is a sequence of algebraic-data declarations followed by typed
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
Gamma is monomorphic and fully annotated. It has no closures, higher-order
functions, mutation, effects, subtyping, or implicit conversion. Algebraic data
is immutable and nominal.

`Int` and `Bytes` are the only built-in data types. Every other type and
constructor is declared in source. Functions and constructors support arbitrary
arity; Beta's register count is not a Gamma language limit.

## Static semantics

The checker resolves every type, function, constructor, variable, and pattern
against its declaration. It checks parameter, function, and constructor arity;
operator operands; call arguments; declared result types; match scrutinees;
pattern constructors and bindings; and agreement among every match arm.

Every match over an algebraic type is exhaustive. Constructors may be covered
directly or by one final catch-all binding. Duplicate constructor arms, an arm
after a catch-all, a constructor from another type, and a missing constructor
reject the program. A checked Gamma program therefore has no dynamic
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

`Int` is a checked signed 64-bit integer. Arithmetic overflow, division by zero,
and signed division overflow trap. `eq` and `lt` on integers produce `0` or `1`;
`if` treats zero as false and every other integer as true.

`Bytes` is an immutable finite byte sequence, not an algebraic list and not a
raw-memory address. The six `bytes_*` forms above are closed built-ins:
`bytes_empty` and `bytes_single` construct; `bytes_length` returns `Int`;
`bytes_get` returns the selected byte as `Int`; `bytes_slice` takes start and
length; and `bytes_concat` joins two sequences. A constructed byte must be in
`0..255`; a negative or invalid index, slice, or length traps. The compiler
may represent sealed input as a flat view and constructed output as chunks or a
rope, but representation and storage coordinates are never Gamma values.

The compact primitive is required by the compiler customer. Representing the
4 MiB input profile as one `Cons(Int, Bytes)` node per byte would require at
least 64 MiB at the current 16-byte row size, while the existing Gamma oracle
has a 16 MiB heap. That mismatch is structural, not an optimization problem.

Divergence remains divergence. Fuel is never Gamma meaning. An evaluator may
bound work for a diagnostic run, but fuel exhaustion is an implementation
profile's `Incomplete` result and proves neither rejection nor divergence.

## Compiler-application profile

Gamma itself has no byte-I/O operation. A Gamma program used as a canonical
compiler publishes one exact entry declaration selected by its accepted-source
profile. The Gamma-written Delta compiler has the shape:

```text
main : Bytes -> DeltaCompileOutcome

DeltaCompileOutcome =
    Complete(Bytes)
  | Reject(DeltaRejectReason, Int)
```

The `Int` in `Reject` is the source byte offset. D17 and
`source/delta/LANGUAGE.md` own the closed `DeltaRejectReason` constructors and
the explicit `DCOUT` constructor-to-wire-code table; codes never derive from
declaration order. A different accepted language owns a different reason sum
and table.

Only the generated Alpha adapter performs I/O. It reads sealed stdin, constructs
the input `Bytes`, invokes `main`, and maps a returned value as follows:

- `Complete(bytes)` validates and writes the exact raw artifact, then halts 0.
- `Reject(reason, offset)` validates the exact reason and
  `0 <= offset <= input length`, writes the accepted-edge diagnostic frame, and
  halts 1.
- private source, heap, stack, output, or adapter exhaustion writes an
  `Incomplete` frame and halts 2.
- a Gamma trap, impossible checked state, adapter contradiction, or replay
  disagreement writes an `InternalFailure` frame and halts 3.

`Incomplete` and `InternalFailure` are adapter outcomes rather than constructors
of `DeltaCompileOutcome`: they occur precisely when pure `main` does not return a
source-authored value. No failure path publishes partial artifact bytes.

## Compiler boundary family

Canonical compiler edges share the boundary discipline settled for the Beta
compiler, not Beta's exact `BCOUT` identity:

- halt values `0..3` mean Complete, Reject, Incomplete, and InternalFailure;
- success stdout is the raw runnable tape with no wrapper;
- failure stdout is one canonical 40-byte, `0xFF`-prefixed frame whose tag agrees
  with the halt value; and
- unknown, malformed, noncanonical, or mismatched frames reject.

Each accepted-language edge owns its magic, version, reason/resource/internal
tables, and coordinate vocabulary. `BCOUT` remains Beta-specific; the Gamma and
Delta compiler edges use their own identities (`GCOUT` and `DCOUT`). One
parameterized decoder may validate all profiles, but no profile may interpret
another profile's frame. Generated-program statuses such as 250 and 251 are
separate runtime observations, not compiler-boundary cases.

## Compilation requirements

The Beta-written Gamma compiler type-checks before emission, erases types into a
defined runtime representation, and emits Alpha tape directly. Its private
frame ABI must support arbitrary function and constructor arity and preserve
proper tail calls. It may not publish an interpreter plus serialized syntax,
invoke an external evaluator, add Gamma operations to Beta or Alpha, or make a
private capacity into Gamma semantics.

`typeck.beta` is reusable static-semantics material and `interp.beta` is a
semantic oracle. Their present output convention--printing a value while also
placing an integer projection in the halt word--is not the compiler boundary.
They are absorbed where economical and otherwise reduced to focused tests or
deleted once the direct compiler edge subsumes their diagnostic roles. The
Python reference is temporary differential scaffolding and never part of the
completed offline bootstrap closure.

The interpreter traps immediately on an unmatched pattern as a temporary
hardening measure. The canonical compiler must instead reject the
nonexhaustive source statically.

## Current oracle coverage

The current oracle gates check the outer source contract before parsing: the
evaluation surface passes 48 focused cases, the typed surface passes 34, and
the temporary independent evaluator agrees on 106 fixed or generated cases.
These counts include CR-terminated comments and fail-closed NUL, vertical-tab,
DEL, and high-byte controls. They cover bounded parts of this contract but do
not constitute the missing compiler edge or establish an obligation both
oracles omit.

The former Gamma proof-kernel copies and the old generic canonical-byte and
terminal-codec prototype were not consumed by a live artifact admission and are
retired to Git history. Future artifact-specific decoding belongs beside the
artifact it admits.

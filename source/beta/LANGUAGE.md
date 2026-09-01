# Beta language

Beta is a strict, first-order functional bootstrap calculus. Its only customers
are the Gamma compiler and named bootstrap tools. General-purpose language
features have no standing.

## Source form

Source bytes are HT, LF, CR, and printable ASCII. Every other byte rejects
before tokenization at its exact byte offset. Space, tab, CR, and LF separate
tokens; `;` begins a comment through the next CR, LF, or source end.

Identifiers match `[A-Za-z_][A-Za-z0-9_]*`. Integers are an optional `-`
followed by decimal digits and must fit signed 64-bit `Int`. A byte literal is
`#x` followed by zero or more pairs of lowercase hexadecimal digits
`[0-9a-f]`. Each pair denotes one byte in source order; bare `#x` denotes empty
`Bytes`. Uppercase digits, separators, quotes, and escapes are not admitted.

```text
program      := form+
form         := (data TYPE constructor+)
              | (def NAME (NAME*) expression)
              | (entry NAME)
constructor  := (CONSTRUCTOR NAT)

expression   := INTEGER | BYTE_LITERAL | NAME
              | (if expression expression expression)
              | (let NAME expression expression)
              | (+ expression expression)
              | (- expression expression)
              | (* expression expression)
              | (/ expression expression)
              | (% expression expression)
              | (= expression expression)
              | (< expression expression)
              | (bytes-single expression)
              | (bytes-length expression)
              | (bytes-get expression expression)
              | (bytes-slice expression expression expression)
              | (bytes-concat expression expression)
              | (NAME expression*)
              | (CONSTRUCTOR expression*)
              | (match expression arm+)

arm          := ((CONSTRUCTOR NAME*) expression)
              | (_ expression)
```

`TYPE` and `CONSTRUCTOR` begin with `A..Z`; `NAME` begins with `a..z` or `_`.
`NAT` is a nonnegative decimal arity. Exactly one `entry` is required and it
must name a function of one parameter. Global type, constructor, and function
names are unique in their grammar-selected namespaces. Constructors are
globally unique because constructor use is unqualified. Parameters, `let`
bindings, and pattern bindings may not shadow an active local.

`Complete` and `Reject` are reserved evaluator-provided constructors with
arities one and zero. Source may neither declare nor bind those names. The entry
function must return `(Complete bytes)` or `Reject`; every other returned value
is an evaluator contradiction. Only `Complete` can publish bytes.

Every source-declared function and constructor has arbitrary finite arity, and
each application must supply that exact declared count. Reserved primitive
forms retain the fixed arities shown by the grammar. Function names occur only
in call position: Beta has no function values. Declarations are mutually
visible, permitting forward and mutual recursion.

Every `match` evaluates its subject once. Its named arms must be an exact
declaration-order prefix of one constructor family. That prefix either covers
the complete family or is followed by one final wildcard arm. A constructor
from another family, a duplicate or reordered arm, a missing arm without a
wildcard, a wildcard before the end, or an arm after the wildcard rejects.
Pattern binder count must equal constructor arity.

Before execution, one complete structural pass validates every source byte and
token, balanced list, top-level declaration, and expression form including each
reserved form's child count. It records declaration/body spans, rejects
duplicate global declarations, and resolves the single entry. It constructs no
AST or bound occurrence graph.

Global and local name resolution, function and constructor arity, duplicate
active local bindings, and match constructor-family/order agreement are checked
only when execution reaches that form. Failure traps explicitly; it is never
ignored or converted into a value. Applying a primitive to the wrong runtime
value kind likewise traps explicitly. Thus malformed unreachable syntax rejects
before execution, while a structurally valid unreachable name or arity mistake
has no effect until reached.

## Values and evaluation

Values are `Int`, immutable `Bytes`, or immutable constructor applications.
Evaluation is strict and left-to-right. `if` evaluates only its selected branch;
integer zero is false and every other integer is true. `match` evaluates only
its selected arm. A call evaluates each argument exactly once before entering
the callee. Tail calls must run without consuming an additional return frame.

Integer addition, subtraction, and multiplication trap when the mathematical
result is outside signed 64-bit range. Division and remainder use truncation
toward zero and trap for zero divisors and `INT64_MIN / -1`. `=` compares
complete values structurally and returns `0` or `1`: unlike kinds and different
constructors are unequal, integers compare numerically, bytes compare logical
contents, and equal constructors compare every field left-to-right. `<` accepts
only `Int`.

`bytes-single` accepts one `Int` in `0..255` and returns that one byte;
out-of-range input traps. `bytes-length` returns a nonnegative `Int`.
`bytes-get` traps on a negative or out-of-range index. `bytes-slice` takes start
and length and traps unless that half-open range is contained in the source.
`bytes-concat` traps before allocation if the exact result length exceeds
`INT64_MAX`. Byte sequences are immutable finite logical sequences;
representation shape is unobservable. Byte sequences are not lists, addresses,
mutable buffers, or host strings.

The entry function receives the invocation's sealed input as one `Bytes` value.
Beta has no ambient input, output, clock, filesystem, process, or foreign-call
operation.

## Evaluator boundary

[`EVALUATOR_PROFILE.md`](EVALUATOR_PROFILE.md) fixes the first audited Alpha
evaluator's exact request bytes, terminal statuses, artifact-publication rule,
spatial bounds, and private representation constraints. Those choices realize
this language but do not add Beta values or expressions. A profile revision
must preserve the language relation or explicitly revise this document.

## Deliberate exclusions

Beta has no mutation, raw memory, closures, higher-order values, macros,
polymorphism, general garbage collector, continuations, exceptions, modules,
packages, interactive evaluation, implicit conversion, subtyping, concurrency,
or ambient effects. A new primitive or form is admitted only when a named
Gamma-compiler or checker workload lowers the complete audited chain cost.

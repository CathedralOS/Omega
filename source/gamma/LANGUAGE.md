# Gamma language

Gamma is a strict, first-order functional bootstrap calculus. Its only customers
are the Delta compiler and named bootstrap tools. General-purpose language
features have no standing.

## Source form

Source bytes are HT, LF, CR, and printable ASCII. Every other byte rejects
before tokenization at its exact byte offset. Space, tab, CR, and LF separate
tokens; `;` begins a comment through the next CR, LF, or source end.

Identifiers match `[a-z_][A-Za-z0-9_]*`. Integers are an optional `-`
followed by decimal digits and must fit signed 64-bit `Int`. A byte literal is
`#x` followed by zero or more pairs of lowercase hexadecimal digits
`[0-9a-f]`. Each pair denotes one byte in source order; bare `#x` denotes empty
`Bytes`. Uppercase digits, separators, quotes, and escapes are not admitted.

```text
program      := form+
form         := (def NAME (NAME) expression)
              | (entry NAME)

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
              | (pair expression expression)
              | (pair-first expression)
              | (pair-second expression)
              | (NAME expression)
```

Exactly one `entry` is required and it must name a declared function. Function
names are globally unique. Every function has exactly one parameter and every
application supplies exactly one argument. Parameters and `let` bindings may
not shadow an active local.

`Complete` and `Reject` are reserved evaluator-provided outcome forms with
arities one and zero. Source may neither declare nor bind those names. The entry
function must return `(Complete bytes)` or `Reject`; every other returned value
is an evaluator contradiction. Only `Complete` can publish bytes.

Reserved primitive forms retain the fixed arities shown by the grammar.
Function names occur only in call position: Gamma has no function values.
Declarations are mutually visible, permitting forward and mutual recursion.

Before execution, one complete structural pass validates every source byte and
token, balanced list, top-level declaration, and expression form including each
form's child count. It records function/body spans, rejects duplicate functions,
and resolves the single entry. It constructs no AST or bound occurrence graph.

Global and local name resolution and duplicate active local bindings are checked
only when execution reaches that form. Failure traps explicitly; it is never
ignored or converted into a value. Applying a primitive to the wrong runtime
value kind likewise traps explicitly. Thus malformed unreachable syntax rejects
before execution, while a structurally valid unreachable name mistake has no
effect until reached.

## Values and evaluation

Values are `Int`, immutable `Bytes`, immutable heterogeneous `Pair` values, or
the reserved outcomes `Complete(Bytes)` and `Reject`.
Evaluation is strict and left-to-right. `if` evaluates only its selected branch;
integer zero is false and every other integer is true. A call evaluates its
argument exactly once before entering the callee. Gamma requires no proper-tail
implementation; a bounded evaluator may report `Incomplete` when its fixed call
storage is exhausted.

Integer addition, subtraction, and multiplication trap when the mathematical
result is outside signed 64-bit range. Division and remainder use truncation
toward zero and trap for zero divisors and `INT64_MIN / -1`. `=` compares
non-pair values structurally and returns `0` or `1`: unlike kinds are unequal,
integers compare numerically, bytes compare logical contents, and outcomes
compare their complete contents. Either operand being a `Pair` traps. `<`
accepts only `Int`.

`pair` evaluates left then right and returns one immutable ordered pair.
`pair-first` and `pair-second` return the corresponding field and trap on every
other value kind. Nested pairs supply the only aggregate-data mechanism.

`bytes-single` accepts one `Int` in `0..255` and returns that one byte;
out-of-range input traps. `bytes-length` returns a nonnegative `Int`.
`bytes-get` traps on a negative or out-of-range index. `bytes-slice` takes start
and length and traps unless that half-open range is contained in the source.
`bytes-concat` traps before allocation if the exact result length exceeds
`INT64_MAX`. Byte sequences are immutable finite logical sequences;
representation shape is unobservable. Byte sequences are not lists, addresses,
mutable buffers, or host strings.

The entry function receives the invocation's sealed input as one `Bytes` value.
Gamma has no ambient input, output, clock, filesystem, process, or foreign-call
operation.

## Evaluator boundary

[`EVALUATOR_PROFILE.md`](EVALUATOR_PROFILE.md) fixes the first Beta-authored
evaluator's exact request bytes, terminal statuses, artifact-publication rule,
spatial bounds, and private representation constraints. Those choices realize
this language but do not add Gamma values or expressions. A profile revision
must preserve the language relation or explicitly revise this document.

## Deliberate exclusions

Gamma has no user-defined algebraic data, pattern matching, arbitrary function
arity, proper-tail guarantee, mutation, raw memory, closures, higher-order
values, macros, polymorphism, general garbage collector, continuations,
exceptions, modules, packages, interactive evaluation, implicit conversion,
subtyping, concurrency, or ambient effects. A new primitive or form is admitted
only when a named Delta-compiler or checker workload lowers the complete audited
chain cost.

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
`#x` followed by an even number of hexadecimal digits and denotes the exact
immutable byte sequence represented by those pairs.

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

Every function and constructor application has exact declared arity. Function
names occur only in call position: Beta has no function values. Declarations
are mutually visible, permitting forward and mutual recursion.

Every `match` evaluates its subject once. Its constructor arms must name every
constructor of exactly one declared type once, in declaration order, or end in
one wildcard arm after a nonempty prefix. A constructor from another type, a
duplicate arm, a missing arm, or an arm after `_` rejects. Pattern binder count
must equal constructor arity.

## Values and evaluation

Values are `Int`, immutable `Bytes`, or immutable constructor applications.
Evaluation is strict and left-to-right. `if` evaluates only its selected branch;
integer zero is false and every other integer is true. `match` evaluates only
its selected arm. A call evaluates each argument exactly once before entering
the callee. Tail calls must run without consuming an additional return frame.

Integer addition, subtraction, and multiplication trap when the mathematical
result is outside signed 64-bit range. Division and remainder use truncation
toward zero and trap for zero divisors and `INT64_MIN / -1`. `=` compares
complete values structurally and returns `0` or `1`; `<` accepts only `Int`.

`bytes-length` returns a nonnegative `Int`. `bytes-get` traps on a negative or
out-of-range index. `bytes-slice` takes start and length and traps unless that
half-open range is contained in the source. `bytes-concat` traps before
allocation if the exact result length exceeds `INT64_MAX`. Byte sequences are
not lists, addresses, mutable buffers, or host strings.

The entry function receives the invocation's sealed input as one `Bytes` value
and returns one value. A customer profile validates that returned value before
publishing bytes or a diagnostic. Expected customer failures are ordinary
source-declared constructors in that returned value, not exceptions. Beta has
no ambient input, output, clock,
filesystem, process, or foreign-call operation.

An authored arithmetic or bytes trap is part of Beta execution. Malformed
private evaluator state is `InternalFailure`. Exhausted source, value, frame,
fuel, input, or output capacity is `Incomplete` under the selected evaluator
profile. Neither implementation outcome is a Beta value or judgment, and no
failure publishes partial output. Divergence remains divergence; fuel does not
reclassify it.

## Deliberate exclusions

Beta has no mutation, raw memory, closures, higher-order values, macros,
polymorphism, general garbage collector, continuations, exceptions, modules,
packages, interactive evaluation, implicit conversion, subtyping, concurrency,
or ambient effects. A new primitive or form is admitted only when a named
Gamma-compiler or checker workload lowers the complete audited chain cost.

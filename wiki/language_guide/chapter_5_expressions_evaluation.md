# Chapter 5: Expressions And Evaluation

Expressions compute values. Statements perform work in a machine or state.

This chapter records the expected evaluation shape so machines, proofs, and
lowering agree.

## Literals

Numeric literals are typed by suffix or context.

```omega
let machine_value: i32 = 3i32;
let proof_value: Nat = 3nat;
let real_value: Real = 3.0real;
```

Machine numeric types such as `i32` and `u64` carry representation and overflow
obligations. Proof numeric types such as `Nat`, `Int`, and `Real` are
mathematical values.

## Assignment

Assignment writes a value into a place.

```omega
self.health = self.health - damage;
```

The assignment is accepted only if the place is mutable, the value is compatible
with the place type, and the resulting facts satisfy the place invariants.

## Calls

Calls evaluate arguments, enter a machine, and receive its result or output
effects.

```omega
let command: Command = self.parser.resolve(&self.line);
self.view.render_room(&self.room);
```

Argument evaluation order should be specified and stable. The initial policy
should be left-to-right because it is easiest to reason about and diagnose.

## Operators

Operators are typed operations.

```omega
let next: u64 = index + 1;
let weak: bool = self.health <= 25;
```

The same operator spelling may exist for machine numbers and proof numbers. The
operand types decide which rules and obligations apply.

Operators should be understood as shorthand for resolved semantic operations,
not as syntax with a completely separate meaning model. In that sense,
`left + right` is conceptually like a call to the appropriate add/concat
operation for the operand meaning in scope.

A fixed operator spelling such as `+`, `[]`, or the range slice `[..]` is
declared with an optional `spelling` clause on a named `operator` declaration.
The named path stays the canonical identity; the spelling is the surface syntax
that resolves to it.

```omega
operator i32::add(left: i32, right: i32) -> i32 spelling +;
```

So `left + right` resolves to `i32::add` for `i32` operands. The public
signature and any proof obligations stay visible on the declaration; only the
primitive lowering hides behind `boundary` when the operator is a boundary
operator.

This model also applies to privileged syntax. `items[index]` should be
understood as an indexing operator, not as raw pointer syntax. `items[1..]`
should be understood as a range-slice operator. Both resolve to a spelled core
`Slice`/`Array`/`Vec` operator whose `requires` clause is the bounds proof
obligation:

```omega
boundary operator Slice::index<T>(items: &[T], index: u64) -> T
spelling []
requires
    index < items.len;

boundary operator Slice::range<T>(items: &[T], start: u64, end: u64) -> &[T]
spelling [..]
requires
    start <= end && end <= items.len;
```

Those operators have a semantic home that users and tools can inspect, while
their boundary primitive implementation is bound through the compiler/runtime
layer.

This chapter only defines ordinary evaluation. Domain-sensitive operator
resolution, if Omega adopts it, belongs to the domains chapter because it
depends on proved semantic facts rather than raw expression syntax.

## Core Collections And Views

Omega should distinguish user-facing core concepts from the low-level carriers
the compiler uses to lower them.

Likely core collection and text concepts:

- `Array[T; N]`: fixed-size owned inline storage.
- `Vec[T]`: owned dynamic contiguous storage.
- `Slice[T]`: borrowed contiguous view over elements.
- `String`: owned string/text storage, with capacity and `push_str`.
- `&string`: borrowed string/text window (`&mut string` for a mutable window).

The exact surface spelling may stay Rust-like for a while:

```omega
let fixed: [Item; 4];
let view: &[Item] = fixed.as_slice();
let text: String;
```

But semantically, `Array`, `Vec`, and `Slice` should be visible core concepts,
not just implicit compiler behavior. `Array` and `Vec` are owners. `Slice` is
the common borrowed view they can produce. Likewise, an owned `String` can
produce a borrowed text window. The owned text type is `String`; a borrowed text
window is its own type spelled `&string` (or `&mut string`). The capitalization
distinguishes owner from window.

The implementation can still use privileged internal carriers. A slice view is
likely lowered as a descriptor such as pointer plus length. A vector is likely
lowered as an owned buffer with pointer, length, and capacity. Those carriers
belong near the boundary/primitive layer, while the public proof and operator
surface belongs to core concepts such as `Slice`.

This means names such as `Slice::Length` should be browsable and documented as
core semantic declarations even if their runtime representation is compiler
managed.

## Indexing And Slices

Indexed access is an ordinary expression form with proof obligations.

```omega
let item: InventoryItem = items[index];
let first: InventoryItem = items[0];
```

Working interpretation:

- `items[index]` resolves to the appropriate indexing operator for the
  collection or view.
- The compiler must prove the index is in bounds for the accessed view.
- Fixed arrays, vectors, and slice views can use the same `[]` surface while
  obtaining the proof from different sources.
- `items[0]` is valid when the current facts prove the view is non-empty.
- Mutable indexing and mutable subslicing have the same bounds proof
  obligations, plus the ordinary borrow-checking obligation that the selected
  element or view is uniquely writable.
- Slice ranges such as `items[1..]` resolve to a range-slice operator that
  creates a new view with a narrower extent and updated facts.
- Text windows expose byte-oriented operations such as `string::byte` and
  `string::range`; character or grapheme indexing must be a separate semantic
  operation because UTF-8 byte positions are not the same as user-visible
  characters.

Omega uses `a..b` for an exclusive range and `a..=b` for an inclusive range.
The open-ended forms `a..`, `..b`, and `..` are also supported. An inclusive
range `a..=b` is defined to mean the same as `a..(b+1)` and is normalized to the
exclusive form internally.

This normalization fixes the proof obligations for a range end:

- An exclusive end `b` must satisfy `b <= len`.
- An inclusive end `b` must satisfy `b < len`. Inclusive-end validity is exactly
  index-`b` validity, because `b` is an indexed position.
- An inclusive non-empty range additionally establishes a `non_empty` fact.

The overflow edge of an inclusive range, such as `..=len-1` when `len` is `0`,
or `..=MAX` where `b+1` would overflow, is a proof error, not a runtime panic.

Omega loops often look like repeated transitions over either:

- a bounded index carried in state parameters, or
- a shrinking slice window where `[0]` remains valid until the window is empty.

The important point is that indexing is not magical pointer syntax. It is a
normal operation guarded by proof of a valid range. For built-in core
collections, the operator contract and named measures should be visible as part
of the core language surface; the low-level pointer/descriptor work belongs to a
boundary primitive implementation layer below that surface.

The visible core declaration should therefore look like a normal contract on a
`boundary operator`:

```omega
boundary operator Array::index<T>(items: &Array<T>, index: u64) -> T
spelling []
requires
    index < items.len;

boundary operator Vec::index<T>(items: &Vec<T>, index: u64) -> T
spelling []
requires
    index < items.len;

boundary operator Slice::index_mut<T>(items: &mut [T], index: u64) -> &mut T
spelling []
requires
    index < items.len;

boundary operator Slice::range_mut<T>(items: &mut [T], start: u64, end: u64) -> &mut [T]
spelling [..]
requires
    start <= end && end <= items.len;

boundary operator Slice::from<T>(items: &[T], start: u64) -> &[T]
requires
    start <= items.len;

boundary operator string::byte(text: &string, index: u64) -> u8
requires
    index < text.len;

boundary operator string::range(text: &string, start: u64, end: u64) -> &string
spelling [..]
requires
    start <= end && end <= text.len;

boundary operator Vec::with_capacity<T>(capacity: u64) -> Vec<T>;

boundary operator String::with_capacity(capacity: u64) -> String;

boundary operator String::push_str(text: &mut String, value: &string) -> ()
requires
    text.len + value.len <= text.capacity;
```

The proof checker owns `start <= items.len`. The boundary primitive owns the
descriptor/pointer rewrite that actually constructs the narrower view.
For allocation-facing contracts such as `Vec::with_capacity` and
`String::with_capacity`, the public core
declaration owns the source meaning while the boundary primitive owns allocator
and buffer initialization details.
Mutating growth operations such as `String::push_str` have both a capacity proof
obligation and a borrow-checking obligation: the string must be uniquely
writable, and any active text window borrowed from it must not be invalidated.

Operator declarations form overload sets by call signature. Overload resolution
keys on the operator path plus parameter types; return type alone never
distinguishes overloads. Generic signatures are compared by structure, not by the
spelling or declaration order of type parameters, so these two declarations
describe the same candidate and must be rejected as a duplicate:

```omega
operator Slice::index<T>(items: &[T], index: u64) -> T;
operator Slice::index<U>(items: &[U], index: u64) -> U;
```

Likewise, `combine<T, U>(left: T, right: U)` and
`combine<A, B>(left: B, right: A)` are the same broad generic call signature:
two independently chosen type parameters.

Distinct parameter types may coexist as an overload set, but resolution must
eventually choose one unique candidate from operand types and proof context.

## Numeric Semantics

Machine numbers and proof numbers are different kinds of values.

Working categories:

- `UInt` is a proof-level natural number.
- `Int` is a proof-level integer.
- `Real` is a proof-level real number.
- `i32`, `u64`, `f32`, and similar types are concrete machine representations.

Proof-level numbers are useful for specifications, constraints, and generic
numeric reasoning. Lowering to native code must erase proof-only numbers or
replace them with proven machine representations.

Machine integer arithmetic is **exact by default**: every operation must be
PROVEN free of overflow, underflow, division by zero, and invalid shifts. If
the compiler cannot prove an operation safe, it is a **compile error** — there
is no unexpected arithmetic and no silent wraparound. (Decided 2026-06-14; this
is the Ada/SPARK model — range types plus a prover — not a build-mode flag.)

To perform arithmetic that *can* overflow, the value lives in an explicit
primitive **domain** that defines the behavior:

- `Wrapping`: wraps modulo the fixed-width representation.
- `Saturating`: clamps to the representable minimum or maximum.
- `Trapping`: checks at runtime and traps on overflow — the escape hatch when
  safety cannot be proven and neither wrap nor saturate is wanted.

Shift counts follow the same rule (settled 2026-07-18): under Exact, a
shift's count must be **proven** below the operand width (a literal
out-of-range shift is an immediate compile error); under `Wrapping` the
count is masked to the width (`k & (width - 1)`) — the genuinely modular
reading, and what the hardware computes anyway; under `Trapping` an
out-of-range count traps. `Saturating` adds no count meaning: it governs
value overflow, not operand validity, so its count obligation is Exact's.
The compiler never adopts the ISA's silent count-masking under Exact —
`x << 64 == x` is an invented number.

### Float-to-integer casts

A float-to-integer cast is also proof-or-policy (settled and implemented
2026-07-18):

- The default Exact cast must prove that the operand is finite and inside the
  target integer's half-open conversion interval. A declared float range can
  supply the proof. A dominating incoming guard can supply it with ordered
  lower/upper comparisons; `x == x` is the explicit witness that excludes NaN.
- `in Saturating` truncates toward zero and clamps at the target width on every
  integer target. NaN converts to zero.
- `in Trapping` truncates an in-range finite value and traps on NaN, infinity,
  or either out-of-range direction.
- `in Wrapping` is a compile error: floats have no modular conversion reading.

These rules are identical in the interpreter and the x86-64/AArch64 bindings;
ISA-specific invalid-conversion sentinels are never language-visible.

Two rules keep it honest:

- **No implicit widening.** `u8 + u8` is a `u8` and must be proven to fit a
  `u8`; to compute in a wider type, cast explicitly (`a as u16 + b as u16`).
- **No mixed-domain arithmetic.** An exact value and a `Wrapping` value cannot
  be combined directly; cross domains with an explicit `as` cast. Explicit
  always wins.

Weaker behavior is therefore always visible at the value, and overflow is a
proof obligation like any other in the language.

### Where the wrap applies: at each node, at the declared width

In a compound expression, a domain-bearing operation produces its
declared-width result **before** the enclosing operation consumes it. With
`a: u32 in Wrapping` holding `0 - 2` (that is, `0xFFFF_FFFE`):

```omega
let b: u32 = a >> 1;    // 0x7FFF_FFFF -- shifts the WRAPPED 32-bit value
let d: u32 = a / 3;     // 1431655764  -- divides the wrapped value
```

The shift and division see the 32-bit wrapped value, never a wider
intermediate (a 64-bit register image of `0 - 2` would shift to garbage).
This holds identically in the interpreter and native emission on both ISAs,
in every operand position, and is pinned by
`arithmetic/runtime_wrapping_operand_truncation_exit`. Exact values need no
such rule (they are proven in-range), and `Saturating`/`Trapping` clamp or
trap at the node before the parent consumes the result.

Two node-level corollaries for constants: a constant stored into a
`Saturating` target clamps to the target's range, and a constant that
provably overflows a `Trapping` target still compiles and traps at runtime
-- Trapping overflow is a runtime event, not a compile error.

## Constants: Two Phases

A constant is either anonymous or landed—never both and never neither.

- **Anonymous (pre-landing):** a literal is an exact mathematical value
  with no type. `100` is one hundred; `3.14` is 157/50. Arithmetic between
  anonymous constants is exact (unbounded integer; `Rat` for decimals).
  No width, no signedness, no domain, no format — deliberately: the value
  is chosen, the machine rendering is not.
- **Landing:** the first site that requests a type renders the value ONCE
  — range-checked into an integer type, rounded once into a float format.
  The same literal lands as `u8`, `u64`, `f32`, or any future format with
  no suffix; a suffix (`0u32`) merely lands the literal where it stands.
- **Landed:** from that point the constant IS a value of its type, and the
  type/domain/format ride with it. All further compile-time folding
  happens at the landed type's semantics — width, signedness, domain
  (a constant that provably overflows a `Trapping` target still compiles
  and traps at runtime, per the node rule above), format rounding.

Nothing is ever both (an anonymous value with a width) or neither (a landed
value stripped of its type). Constant folding must preserve the landed type,
domain, and format; it cannot regress a landed value to an untyped integer.

## Float Facts

> A float is a format-parameterized approximation carrier: every operation is
> "exact
> rational arithmetic, then round once" under a FORMAT the target binds.
> `f32`/`f64` name the IEEE binary32/64 formats — a fact recorded in target
> provides data, never in the grammar. Every finite float is exactly a
> dyadic rational, so float facts are `Rat` facts (chapter 10). Names mean
> formats, always: `f32` never rebinds to a different representation on any
> target; a future format (posits, bf16) arrives as a new name plus a
> format record plus provides rows — zero grammar.

Float constraints describe correctness facts, not optimization permissions.

### Value domains — wellness facts

```omega
data Particle {
    x: f64;                            // bare: may hold NaN/±inf quietly
    speed: f64 in Finite;              // NaN and ±inf forbidden
    alpha: f32 [0.0..=1.0];            // range fact — implies Finite for free
    mass: f64 in Finite & Positive;    // domains conjoin with the landed `&`
}
```

- `Finite` (core domain; ch5's original `finite` constraint, promoted):
  the value is not `NaN`, `+inf`, or `-inf`. Enforcement is the
  invariant-window machinery (chapter 11): writes are free, the window
  closes at consumption points.
- A float range (`0.0f..=100000.0f`) is a window-checked value fact — and
  every range implies `Finite`: NaN fails every comparison, so no range
  admits it.
- A domain chain `in A & B & C` carries any number of value domains and
  **at most one** policy domain (two policies is the existing
  mixed-domain rejection).
- Float constraints are not runtime metadata.

### Policy domains — operation behavior

Operand-driven and exclusive per operation (the decision-17 rule, applied
to floats — whose failure modes are non-finite production, not overflow
into wraparound):

- **default**: the format's quiet semantics — correctly rounded, inf/NaN
  propagate silently; `Finite` windows catch them wherever wellness was
  claimed.
- **`in Trapping`**: producing a non-finite value traps.
- **`in Saturating`**: overflow clamps to the format's largest finite
  magnitude — **overflow only** (settled 2026-07-18): division by zero and
  invalid operations (`0.0/0.0`, `inf - inf`, `sqrt` of a negative) still
  produce non-finite values; those routes remain `Finite` obligations.
  `Finite & Saturating` is therefore the ergonomic pairing: magnitude
  proofs vanish into the clamp, wellness stays proven via the cheap
  discrete obligations (divisor nonzero, operand signs).
- **`in Wrapping`**: compile error — there is no modular reading of a
  float (the float-to-int cast ruling's precedent, generalized).

### Float comparisons and NaN

Float comparisons follow the format's partial order on both engines (IEEE
under the binary32/64 bindings):

- The ordered comparisons `<`, `<=`, `>`, `>=` are **false** whenever either
  operand is NaN (natively: aarch64 condition codes chosen to fail on
  unordered; x86_64 `ucomis*` with parity-aware sequencing).
- `==` is false and `!=` is **true** when either operand is NaN — so
  `f != f` is the IEEE binding's isNaN idiom. The constant folder
  deliberately refuses to fold float self-comparisons (`x == x`, `x != x`)
  for exactly this reason. `is_finite(x)` is the portable wellness
  spelling; the idiom is what implements it under IEEE bindings.

`min` and `max` follow the **hardware contract** (settled 2026-07-18):
return the second operand on unordered-or-equal — exactly `a < b ? a : b`,
matching `minsd`/`maxsd` and the aarch64 FCSEL lowering. This is
order-dependent under NaN (`min(NaN, 5)` is `5`; `min(5, NaN)` is `NaN`)
and deliberately differs from both Rust (non-NaN wins — which silently
launders a poisoned value) and IEEE-2019 `minimum` (NaN wins — which costs
a compare-and-blend on x86). Under `Finite` operands all three contracts
agree, so proven code cannot observe the choice; unproven code gets the
fastest true-to-silicon lowering.

Comparison results in value position (`let ok: bool = a > b`) use the same
lowering as guards and are pinned for ordinary values — including negative
operands, where a bit-pattern comparison would invert the order — by
`arithmetic/runtime_float_compare_bool_exit`. NaN-operand differential legs
are now pinnable (runtime `0.0 / 0.0` under the quiet default constructs
NaN portably).

NaN payload bits are unspecified after every operation and never
proof-observable; a recast (`&self.f as &u32`) reads whatever bits are
honestly there.

### Two orders

Arithmetic comparison is the partial order above — floats never pretend to
be totally ordered in arithmetic position. Sorting and keying use a total
order spelled as a named satisfier (chapter 14):

```omega
sort_by<F64::TotalOrder>(&mut samples);   // IEEE totalOrder — a sign-magnitude
                                          // integer compare, explicit at the site
```

### No ambient relaxation

- `a * b + c` is two roundings, on every target, always — the compiler
  never contracts multiplies into fused ops on its own.
- `fma(a, b, c)` is the single-rounding spelling.
- There is no fast-math mode, flag, or build option, and none is planned.
  Optimization permissions, where they ever exist, are per-operation
  spellings — never ambient. (This settles the two-layer question this
  section used to carry: the permission layer is spelled ops.)

### Literals and compile-time arithmetic

A float literal parses to its exact rational value (`9.80665` IS
`196133/20000`); compile-time float arithmetic is exact `Rat` arithmetic;
the result rounds **once**, at the landing site, to the landing type's
format. The same literal lands correctly as `f32`, `f64`, or any future
format with no suffix — a constant is unitless until a site requests a
type. Where the exact operation is undefined or exceeds the format
(division by zero, overflow), compile-time evaluation applies the format's
special-value semantics — so compile-time and runtime agree bit-for-bit by
construction, NaN production included.

## Temporaries

Temporaries have lifetimes like locals generated by the compiler.

```omega
self.view.render_line(RoomFormatter::title(&room));
```

The checker must ensure temporary storage outlives every borrow derived from it
and is cleaned up on every exit edge.

When lifetime or proof facts become hard to read, prefer explicit locals.

## Blocks

A block groups straight-line work.

```omega
{
    let room: Room;
    self.lookup.find_room(self.level, self.current_cell, &mut room);
    self.view.render_room(&room);
}
```

The block can introduce locals and cleanup edges. It does not create a state or
machine by itself.

## Terminal Expressions

In a typed machine, a terminal expression completes the current machine
invocation.

```omega
state shutdown(&mut self) {
    0
}
```

A transition target uses call-shaped syntax. A bare value is a terminal value.

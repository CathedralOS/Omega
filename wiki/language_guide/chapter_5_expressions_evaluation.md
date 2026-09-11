# Chapter 5: Expressions And Evaluation

Expressions compute values; statements perform work in a machine or state.
The [expression specification](../spec/language/expressions.md) defines their
schedule and operation selection. This chapter shows how those rules affect
ordinary code. Examples describe the language contract, not complete compiler
support; current numeric limits are recorded [beside validation](../../omega-rust/psi/semantics/validation/numeric_proofs.md).

## Literals

A suffix chooses a numeric type at the literal. Otherwise the value stays
anonymous until an operation or destination requests its type:

```omega
let machine_value: i32 = 3i32;
let proof_value: Nat = 3nat;
let machine_float: f64 = 3.0f64;
```

Machine types such as `i32`, `u64`, `f32`, and `f64` carry representation and
arithmetic obligations. `Nat`, `Int`, and `Real` express mathematical values.
`Real` is an ordinary proof-side abstraction, not a runtime float format or
compiler numeric primitive.

## Evaluation Schedule

Calls evaluate their receiver first, then arguments left to right. Strict
binary operators evaluate left before right. Each evaluated child runs once.
`&&` evaluates its right side only after a true left side; `||` does so only
after a false left side. A transition evaluates its subject once and runs only
the selected arm.

For named aggregate fields, the literal's order matters:

```omega
Pair { second: make_second(), first: make_first() }
```

This calls `make_second()` first, regardless of the declaration or byte layout
of `Pair`. Array elements instead run in increasing index order. Abandoning
partial construction on an ordinary cleanup edge disposes the established
prefix in reverse order; completed aggregates follow their structural cleanup
rule. Trap and Abort routes have no cleanup successor.

The [complete schedule](../spec/language/expressions.md#evaluation-schedule)
also covers indexing, ranges, short-circuit facts and legal reordering.

## Assignment

Assignment writes into a place:

```omega
self.health = self.health - damage;
```

The place must be writable and the value compatible with its type. The
applicable [invariant window](chapter_11_invariant_windows.md)
must close with the required facts; assignment does not preserve facts about
the old contents automatically.

## Calls

```omega
let command: Command = self.parser.resolve(&self.line);
self.view.render_room(&self.room);
```

Calls whose known envelope permits suspension or worker blocking require exact
acknowledgements:

```omega
ordinary_call();                // guaranteed neither
suspend may_park();             // may suspend the activation
block may_block_worker();       // may block the current worker
suspend block may_do_either();  // may do either
```

The markers acknowledge possibilities, not events that must happen. They do
not select an implementation or create a task. Missing or redundant markers
reject. Imports and requirements use their pinned envelopes, not a narrower
implementation guessed by the caller.

A suspending call must be a complete statement, simple `let` right-hand side,
transition subject, or terminal expression. Bind its result before combining it:

```omega
// Rejected: a partial expression would become hidden continuation state.
let total: u64 = prefix + suspend source.next();

let next: u64 = suspend source.next();
let total: u64 = prefix + next;
```

A blocking-only call may nest because it keeps its stack rather than creating
a continuation boundary. A separate local is still often clearer. See
[call-site acknowledgements](../spec/language/effects.md#call-site-acknowledgements).

## Operators

An operator token selects an ordinary named machine with a visible contract.
Write the token immediately after `machine`; there is no separate operator
declaration keyword. For example, with `Vec2` and a checked `add_vectors` helper:

```omega
pub machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 {
    add_vectors(left, right)
}
```

Operand types and static domains choose the meaning of `left + right`.
The token is shorthand, not a second semantic system. Multiplication, division
and remainder bind more tightly than addition and subtraction; each of these
binary tiers associates left. Parentheses change grouping, not evaluation order.

Token-bearing machines can have attached receivers or explicit operands. These
are alternative declarations, not competing bindings to publish together:

```omega
machine + Vec2::add(self, right: Vec2) -> Vec2 {
    add_vectors(self, right)
}

machine == Vec2::equals(&self, other: &Vec2) -> bool {
    equal_vectors(self, other)
}
```

The receiver occupies operand position zero with its ordinary ownership meaning.
An operator grants no special permission to mutate. A path-qualified call such
as `Token::ordered(left, right)` uses a static namespace, not a value receiver.

A trait can publish an operator requirement:

```omega
trait Ranked<T> {
    machine < compare(left: T, right: T) -> bool;
}
```

Its implementation comes from an explicitly selected conformance, never the
only visible candidate by accident. Use an explicit named requirement call
when several meanings are relevant. The [operator declaration and family
rules](../spec/language/expressions.md#operator-declarations) define visibility,
overload identity, fixed vocabulary, and domain ownership.

### Who supplies the body?

| Declaration | Executable supply |
| --- | --- |
| Ordinary `machine + Vec2::add(...) { ... }` | Its own checked body; delegating to another machine is an ordinary call. |
| `machine < compare(...);` inside a trait | An explicitly selected conformance. |
| `boundary machine + Float::add(...);` | The existing admitted boundary provider. |
| Exact compiler-owned `machine Float::meaning32(...);` | Its automatic closed-catalog implementation. |

The last two cases are different:

```omega
// Canonical core declarations, not user opt-in spellings.
pub boundary machine + Float::add(left: f32, right: f32) -> f32;
machine Float::meaning32(value: f32) -> FloatMeaning;
```

The complete core float contracts still govern these abbreviated signatures.
Target defaults supply float addition; `build.omg` may select another admitted
implementation satisfying the same arithmetic contract. It does not define what
addition means. The meaning projection is a compiler primitive and needs no
build selection; its visibility and proof-only restrictions still apply.

A bodyless ordinary direct declaration outside a requirement context or the exact
compiler catalog rejects. Another machine writing `satisfies` does not select a
body for it. A same-spelled declaration in another package cannot impersonate a
compiler primitive.

Closed direct families belong to their semantic-home owner. The owner checks
duplicate participating token/operand shapes within its declaration set; foreign
packages cannot inject another `+` into `Vec2`. They may use their own adapter
type/domain or an explicitly selected trait conformance. Primitive families have
toolchain-designated core owners. A package owning `Degrees` may nevertheless
define its domain's arithmetic over `f64` without replacing bare float addition.
Unrelated imports cannot change an existing selection or introduce a collision.

## Core Collections And Views

Fixed arrays and vectors own storage; slices borrow it. UTF-8 qualifies the
existing byte carrier rather than introducing a separate text owner hierarchy:

```omega
let fixed: [Item; 4];
let view: &[Item] = fixed.as_slice();
let text: [u8; 5] in Utf8 = "hello";
let text_view: &[u8] in Utf8 = text;
```

`Array`, `Vec`, and `Slice`, including named measures such as `Slice::Length`,
are visible core concepts. A slice projection changes the view descriptor
without copying initialized bytes. The owner cannot be reallocated while a
view borrows its storage; mutations must respect the selected loan and extent.
Pointer/length and pointer/length/capacity carriers are implementation details.

Growable text uses the same `Vec<u8>` carrier qualified by `Utf8`. Dynamic
storage comes from an explicit allocator over qualified storage authority,
not an ambient `Vec::with_capacity`. Appending must establish capacity,
encoding, and unique-borrow obligations.

## Indexing And Slices

```omega
let item: InventoryItem = items[index];
let first: InventoryItem = items[0];
```

Both accesses need bounds evidence; `[0]` needs a nonempty view. Indexing and
slicing are ordinary operators, with browsable contracts such as:

```omega
boundary machine [] Slice::index<T>(items: &[T], index: u64) -> T
requires
    index < items.len;

boundary machine [..] Slice::range<T>(items: &[T], start: u64, end: u64) -> &[T]
requires
    start <= end && end <= items.len;
```

The checker establishes bounds; the selected implementation constructs the
descriptor. Mutable indexing adds the appropriate exclusive loan—it does not
replace the bounds proof. A live `items[1..]` need not prevent a write to
`items[0]`, but an overlapping write to `items[1]` conflicts.

`a..b` excludes `b`; `a..=b` includes it. Open forms are `a..`, `..b`, and `..`.
An inclusive end is an indexed position, so it needs `b < len`, rather than
the exclusive end's `b <= len`. Its exclusive successor must not overflow.
For example, `..=len-1` needs nonzero length. Invalid bounds are proof errors,
not an implicit runtime panic.

Text ranges count bytes. Character and grapheme indexing need separate
operations. See [range formation](../spec/language/expressions.md#indexing-and-ranges)
and [collection bounds](../spec/language/numeric_values.md#collection-bounds).

## Numeric Semantics

Machine integer arithmetic is Exact by default: each operation must be proved
free of invalid operands and unrepresentable results. A safe final result
does not excuse an overflowing intermediate. There is no implicit widening;
`u8 + u8` requires a `u8` result unless an explicit conversion chooses a wider
operation.

For intentionally different overflow behavior, choose one policy:

- `Wrapping` reduces overflow modulo the carrier width.
- `Saturating` clamps overflow to the carrier bounds.
- `Trapping` checks at runtime and traps at the primitive's failure edge.

These policies do not all repair invalid operands: wrapping and saturation
never license integer division by zero. Policies compose with independent
meaning, such as `Km & Wrapping`, but not with another arithmetic policy.
Qualification changes future operations, not the existing payload.

Bitwise operations preserve their integer carrier; Boolean `!` is distinct from
integer `~`. Exact shifts prove a nonnegative count below the value width.
Wrapping reduces the count modulo that width, Trapping checks it, and Saturating
keeps the Exact count obligation. Hardware masking is not an Exact proof.
The full [numeric rules](../spec/language/numeric_values.md#arithmetic-and-comparisons)
also distinguish value overflow from operand validity.

### Float-to-integer conversion

Exact `as` needs an integral, representable value and preserves its denotation.
Use a named conversion to request fractional truncation. An unqualified
float-to-integer conversion truncates toward zero and requires finite input
with a representable truncated result. Saturating conversion clamps and maps
NaN to zero; Trapping conversion traps on non-finite or out-of-range input.
Wrapping has no float-conversion meaning.

### Public numeric-conversion requirements

Conversion names belong to their destination:

```omega
F32::from_f64(value)  // nearest-even to binary32
F32::from_i64(value)  // nearest-even to binary32
I32::from_f64(value)  // toward-zero to i32
```

Result-domain overloads choose the policy:

```omega
let exact: i32 = I32::from_f64(value);
let checked_at_runtime: i32 in Trapping = I32::from_f64(value);
let clamped: i32 in Saturating = I32::from_f64(value);
```

Without an expected result, the unqualified overload applies and its proof
obligations remain. A predicate such as `Finite` adds a later proof obligation,
not another overload. Directed one-step conversions have separate names;
rounding then converting is not interchangeable if it causes double rounding.
Failure-returning conversion names remain unsettled.

`as` never calls arbitrary user code, rounds, allocates, or silently chooses a
policy. Unit-scale conversion is an ordinary domain-library operation. See
[destination-owned conversion](../spec/language/numeric_values.md#destination-owned-conversion)
and [exact coercion](../spec/language/domains.md#exact-coercion-and-erasure).

### Where the wrap applies: at each node, at the declared width

Suppose `a: u32 in Wrapping` holds `0xFFFF_FFFE`. Its parent operations see that
wrapped 32-bit value, not an unbounded subtraction or a wider register image:

```omega
let shifted: u32 in Wrapping = a >> 1; // 0x7FFF_FFFF
let divided: u32 in Wrapping = a / 3; // 1431655764
let exact: u32 = divided as u32;       // explicit policy erasure
```

Saturating and Trapping likewise act at each already-landed operation before
the parent consumes its result. A known Trapping overflow remains a runtime
trap. This does not let an anonymous initial value bypass landing: assigning
an out-of-range literal to a Saturating target is not a saturating conversion.

## Constants: Two Phases

An anonymous constant has an exact mathematical value but no selected machine
representation. `3.14` is exactly `157/50`. Landing chooses a type once: integer
landing checks integrality and range; floating landing rounds once.

After landing, all later folding obeys that type, format and domain. It cannot
turn a landed value back into an anonymous integer. A machine parameter is a
landing boundary even when the call is evaluated during compilation.

### Exact anonymous division and landing

Anonymous `/` is rational division, not truncating integer division:

```omega
let exact_integer: i32 = 7 / 2 * 2;    // 7; fractional-intermediate warning
let fractional: i32 = 7 / 2;          // error: 7/2 is not an integer
let typed_integer: i32 = 7i32 / 2 * 2; // 6; typed division, no warning
let exact_float: f64 = 7 / 2 / 2;     // 1.75; no integer-landing warning
```

An already-typed operand instead requests typed arithmetic. With `i: i32`:

```omega
let mixed: i32 = i * 4097 / 2;
let fraction_operand: i32 = i * (4097 / 2);     // error: fractional i32 operand
let truncated_operand: i32 = i * (4097i32 / 2); // i * 2048
```

The first line still owes its intermediate overflow proof. Parentheses cannot
make the final destination retroactively type an anonymous subexpression, and
an optimizer cannot reassociate these into the same computation. Anonymous
division by zero has no value, whatever destination policy is requested.

### Fractional-intermediate diagnostic

The warning helps catch an apparent integer align-down that actually cancels
an exact fraction:

```omega
let base: u32 = (4097 / 4096) * 4096;        // 4097; warning
let aligned: u32 = (4097u32 / 4096) * 4096; // 4096; explicit integer division
```

A fractional final value cannot land as an integer. An integral final value
can, but a fractional anonymous intermediate triggers a default-on warning.
Suppression changes no arithmetic; typed integer division and float landing
do not trigger it. The [diagnostic contract](../spec/language/numeric_values.md#fractional-intermediate-diagnostics)
retains the fractional source origin even after cancellation.

### Typed integer quotient and remainder

Builtin `%` needs an already-integer-typed operand, not merely an integer
destination:

```omega
let anonymous: i32 = -3 % 2;    // error: type an operand
let remainder: i32 = -3i32 % 2; // -1
let positive: u32 = 7 % 2;     // still missing an integer-typed operand
let explicit: u32 = 7u32 % 2;  // 1
```

The same convention applies to proof `Int`: `-7` divided by `2` has quotient
`-3` and remainder `-1`. Quotients truncate toward zero; nonzero remainders have
the dividend's sign. This is not Euclidean modulo. Both operations require a
nonzero divisor; fixed-width Exact also excludes signed `MIN / -1` and
`MIN % -1`. See the [paired quotient/remainder laws](../spec/language/numeric_values.md#integer-quotient-and-remainder).

### Landed target-semantic dependencies

A target-dependent constant may stay symbolic until target closure. Folding
it afterward does not erase the observation or selected-realization inputs
from its signature, cache, proof or artifact. See
[target-dependent applications](../spec/language/evaluation.md#target-dependent-applications).

## Float Facts

`f32` and `f64` permanently mean binary32 and binary64. A target realizes
those meanings; it cannot redefine them. Finite arithmetic computes exact
meaning and rounds through the selected format, with separate special-value
rules for zeros, infinity and NaN.

### Operators are requirements

Float operators use the ordinary resolver and selected provider plans:

```text
a + b
  -> selected Float::add requirement
  -> native instruction or checked software realization
```

Their contracts use executable `FloatSemantics`. Proof-only `FloatMeaning`
distinguishes finite nonzero rationals, signed zeros, signed infinities and one
payloadless NaN case. Its structural equality is not IEEE runtime comparison:
the NaN proof case equals itself, while the two signed-zero proof cases differ.
It has no runtime tagged-union ABI. See
[float meaning](../spec/language/numeric_values.md#meaning-comparisons-and-representation).

### Value domains — wellness facts

```omega
data Particle {
    x: f64;
    speed: f64 in Finite;
    alpha: f32 [0.0..=1.0];
    mass: f64 in Finite & Positive;
}
```

Bare floats may contain NaN or infinity. `Finite` excludes them, and a finite
range implies `Finite`. These are invariant-window facts, not runtime metadata
or optimization permissions.

### Policy domains — operation behavior

Quiet float semantics allow non-finite results. Trapping checks the semantic
result and traps on NaN or infinity. Saturating clamps finite-operand magnitude
overflow, but does not repair division by signed zero or invalid operations
such as `0.0f64 / 0.0`. Wrapping has no float candidate.

Finite inputs alone therefore do not justify this result:

```omega
machine divide(a: f32 in Finite, b: f32 in Finite) -> f32 in Finite {
    return a / b;
}
```

The divisor must exclude both signed zeros and the rounded quotient must fit
the finite range. Saturating removes the magnitude-overflow obligation, not
the divisor obligation. Underflow to signed zero remains finite.

### Float comparisons and NaN

With a NaN operand, ordered comparisons and `==` are false; `!=` is true.
Thus `x != x` is an IEEE NaN test, not an integer-style contradiction.
`is_finite(x)` is the portable wellness query.

`minimum(a, b)` selects `a` only for `a < b`; `maximum(a, b)` selects it only
for `a > b`. Both return the second operand on unordered or equal inputs,
including signed zeros. Operand order therefore matters with NaNs.

Runtime recasts expose honest stored bits, but base arithmetic promises no
reproducible NaN payload. Proof use can retain payloadless NaN meaning; runtime
constant materialization must instead fix the representation through
canonicalization, explicit bits or an exact selected realization. See
[constant materialization](../spec/language/constants.md#materialization).

### Two orders

Sorting selects total order explicitly:

```omega
sort_by<F64::TotalOrder>(&mut samples);
```

Core `F32::TotalOrder` and `F64::TotalOrder` are ordinary named `Order`
conformances over honest representation bits. They do not change arithmetic's
partial order.

### No ambient relaxation

`a * b + c` has two roundings; named FMA has one. The compiler never contracts
the former silently. There is no ambient fast-math option; any future relaxed
operation would need its own explicit contract.

### Literals and compile-time arithmetic

`9.80665` denotes exactly `196133/20000` until landing. Anonymous arithmetic
rounds once into the destination float format; already-landed arithmetic uses
that format at each operation. Compile-time execution and runtime contracts
share `FloatSemantics`, not the host compiler's preferred floating type.

### Canonical floating control state

Checked activations use nearest-even, gradual underflow and masked exceptions.
Directed rounding is a separately named operation, never an ambient mode;
Trapping checks results rather than unmasking hardware exceptions.

Foreign bindings must preserve or restore semantic controls, and callbacks
establish Omega controls before checked code and restore foreign controls on
return. Sticky status flags are outside this invariant. The
[realization note](../../omega-rust/omega/compiler/compiler/float_realization.md)
separates current provider support, image replay and native execution evidence.

## Temporaries

```omega
self.view.render_line(RoomFormatter::title(&room));
```

Temporary storage must outlive its derived borrows and receive ordinary
cleanup. Shared lending may be implicit for a shared-reference parameter;
mutable and write-only lending use explicit `&mut` and `&write`. Prefer a
named local when the lifetime or proof facts become difficult to see.

## Blocks

A block groups straight-line work and introduces local scope and cleanup edges:

```omega
{
    let room: Room;
    self.lookup.find_room(self.level, self.current_cell, &mut room);
    self.view.render_room(&room);
}
```

It does not create a state or machine.

## Terminal Expressions

A terminal value completes the current invocation:

```omega
state shutdown(&mut self) {
    0
}
```

A call-shaped transition target instead transfers within the machine.

# Chapter 5: Expressions and evaluation

Expressions compute values; statements perform work in a machine.
The [expression specification](../spec/language/expressions.md) defines their
schedule and operation selection. This chapter shows how those rules affect
ordinary code.

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

## Evaluation schedule

Static calls evaluate their explicit arguments left to right. Strict
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
player.health = player.health - damage;
```

The place must be writable and the value compatible with its type. The
applicable [invariant window](chapter_11_invariant_windows.md)
must close with the required facts; assignment does not preserve facts about
the old contents automatically.

The destination is selected **before** the right-hand side runs:

```omega
machine replacement(index: &mut u64) -> u64 {
    index = 1;
    19
}

machine example() crashes Trap {
    let mut values: [u64; 2] = [11, 7];
    let mut index: u64 = 0;
    values[index] = replacement(&mut index);
    // values is [19, 7]; index is 1.
}
```

The assignment captures element zero, calls `replacement`, then stores `19`
in that captured element. It does not read `index` again to choose a different
destination. This also applies to cleanup of a displaced owned value. The
right-hand side may change the selector, but must not invalidate the captured
backing under the ordinary ownership and borrowing rules.

## Calls

```omega
let command: Command = Parser::resolve(&parser, &line);
View::render_room(&mut view, &room);
```

The qualified name selects the machine; context is an ordinary argument.
Dot syntax projects a data field, not an implicit machine receiver.

Calls whose known envelope permits suspension or worker blocking require the
corresponding acknowledgements:

```omega
ordinary_call();                // guaranteed neither
suspend may_park();             // may suspend the activation
block may_block_worker();       // may block the current worker
suspend block may_do_either();  // may do either
```

The markers acknowledge possibilities, not events that must happen. They do
not select an implementation or create a task. Missing required markers reject;
extra markers are permitted without a warning and do not create effects.
Imports and requirements use their pinned envelopes, not a narrower
implementation guessed by the caller.

A call written with `suspend`, even redundantly, must be a complete statement,
simple `let` right-hand side, transition subject, or terminal expression. The last category includes a
[final machine call in a transition arm](chapter_4_nested_machines_transitions.md#final-calls-including-suspension),
with ordinary call markers, result forwarding and cleanup. Bind its result
before combining it:

```omega
// Rejected: a partial expression would become hidden continuation state.
let total: u64 = prefix + suspend Source::next(&mut source);

let next: u64 = suspend Source::next(&mut source);
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
public machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 {
    add_vectors(left, right)
}
```

Operand types and static domains choose the meaning of `left + right`.
The token is shorthand, not a second semantic system. Multiplication, division
and remainder bind more tightly than addition and subtraction; each of these
binary tiers associates left. Parentheses change grouping, not evaluation order.

Token-bearing machines also use explicit parameters. Borrowing belongs to the
operand's declared access mode:

```omega
machine == Vec2::equals(left: &Vec2, right: &Vec2) -> bool {
    equal_vectors(left, right)
}
```

An operator grants no special permission to mutate. A path-qualified call such
as `Vec2::equals(&left, &right)` supplies both operands explicitly; qualification
identifies the semantic owner without adding a receiver.

A trait can publish an operator requirement:

```omega
trait Ranked<T> {
    machine < compare(left: T, right: T) -> bool;
}
```

Its implementation comes from an explicitly selected conformance, never the
only visible candidate by accident. Generic code selects that implementation in
its static telescope, for example `Order satisfies Ranked<Element>`; the selected
row then determines `<` inside that generic body. Build provider selection may
realize a boundary requirement but cannot act as `Order`, activate a token, or
change source resolution. Use an explicit named requirement call when several
meanings are relevant. The [operator declaration and family
rules](../spec/language/expressions.md#operator-declarations) define visibility,
overload identity, fixed vocabulary, and domain ownership.

The owner of a type or domain supplies its direct operators. Generic code can
instead select a named trait conformance. This keeps an unrelated import from
changing existing arithmetic. See [operator declarations](../spec/language/expressions.md#operator-declarations)
for overloads and [Chapter 14](chapter_14_traits.md) for selected implementations.

## Core collections and views

Fixed arrays and vectors own storage; slices borrow it. UTF-8 qualifies the
existing byte carrier rather than introducing a separate text owner hierarchy:

```omega
let fixed: [Item; 4];
let view: &[Item] = ArrayAccess::as_slice(&fixed);
let text: [u8; 5] in Utf8 = "hello" as +Utf8;
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

## Indexing and slices

Indexing supplies the collection to a static machine's first parameter:

```omega
data Buffer {
    bytes: [u8; 4];
}

machine [] at(buffer: &Buffer, index: u64) -> u8
requires index < 4
{
    buffer.bytes[index]
}

machine first(buffer: &Buffer) -> u8 {
    buffer[0]
}
```

`buffer[0]` supplies the shared collection loan required by `at`;
no `(&buffer)[0]` spelling is required. A mutable collection parameter requires
exclusive access and forms the corresponding loan. Conflicting loans still
reject. The index itself follows its parameter's ordinary ownership rules.

This is token-specific collection adaptation for `[]` and `[..]`, not receiver
syntax or automatic borrowing of every operator operand. The named call remains
`at(&buffer, 0)`; a mutable named call requires an explicit `&mut` argument.
Index machines may return computed values, as above, or borrowed views; a
value-returning getter does not by itself provide an assignable indexed place.

```omega
let item: InventoryItem = items[index];
let first: InventoryItem = items[0];
```

Both accesses need bounds evidence; `[0]` needs a nonempty view. Copying an
element also requires an unrestricted element type; borrowing an element has
its own access and lifetime obligations.

Core arrays and slices also expose indexing through contracted operations.
Reading an element requires `index < items.len`; mutable access additionally
requires an exclusive borrow.

A caller can establish that requirement through its own contract:

```omega
machine read_at(items: &[u8], index: u64) -> u8
requires index < items.len {
    items[index]
}
```

The precondition supplies the bound for `items[index]`. A branch or checked
theorem can establish it too. A live `items[1..]` can coexist with a write to
`items[0]`, while an overlapping write to `items[1]` conflicts.

`a..b` excludes `b`; `a..=b` includes it. Open forms are `a..`, `..b`, and `..`.
An inclusive end is an indexed position, so it needs `b < len`, rather than
the exclusive end's `b <= len`. Its exclusive successor must not overflow.
For example, `..=len-1` needs nonzero length. Invalid bounds are proof errors,
not an implicit runtime panic.

Text ranges count bytes. Character and grapheme indexing need separate
operations. See [range formation](../spec/language/expressions.md#indexing-and-ranges)
and [collection bounds](../spec/language/numeric_values.md#collection-bounds).

### Creating and forwarding mutable views

Creating a mutable subrange explicitly requests a mutable borrow:

```omega
machine clear(window: &mut [u8])
requires window.len > 0
{
    window[0] = 0;
}

machine edit(view: &mut [u8])
requires view.len >= 4
{
    let middle: &mut [u8] = &mut view[2..4];
    clear(middle); // Forward the existing mutable reference.
}
```

Without `&mut`, the range is a shared view. Writing
`let middle: &mut [u8] = view[2..4];` rejects: the annotation does not request
a mutable borrow implicitly. The same rule applies when passing a range
directly, returning it, passing it to a machine or storing it in a field.
Both forms retain the backing's bounds and borrowing restrictions.

## Numeric semantics

Numeric helpers such as `min`, `max`, `abs`, `clamp` and `sqrt` belong to the
standard library's math machines. Import their declarations and call them like
other machines; these names are not compiler builtins. A caller uses their
`requires` and `ensures`, including bounds needed by a subsequent narrowing
conversion. The declared contract also determines exceptional numeric cases;
do not assume a signed `abs` can represent every magnitude or that all floating
minimum operations treat NaNs the same way. See
[library numeric helpers](../spec/language/numeric_values.md#library-numeric-helpers).

Machine integer arithmetic is Exact by default: each operation must be proved
free of invalid operands and unrepresentable results. A safe final result
does not excuse an overflowing intermediate. There is no implicit widening;
`u8 + u8` requires a `u8` result unless an explicit conversion chooses a wider
operation.

For intentionally different overflow behavior, name a policy at the operation
with a suffix on the operator:

```omega
let next: u64 = cursor +! width;   // traps on overflow
let mixed: u32 = hash *% 31;       // wraps modulo 2^32
let raised: u8 = level +| step;    // clamps to 0..=255
```

- `%` (Wrapping) reduces overflow modulo the carrier width.
- `|` (Saturating) clamps overflow to the carrier bounds.
- `!` (Trapping) checks at runtime and traps at the primitive's failure edge.
  After it returns, the result is the exact value and the checker knows it.

The suffixes apply to `+`, `-`, `*`, `/`, `%`, `<<` and `>>`. Operands and
results stay plain integers, so the next operation is Exact again unless it
names a policy too. A `!` operation is a crash site: its machine's crash
contract must admit `Trap` unless facts already rule the failure out.

When every operation on a value should use one policy, qualify the value
instead: `u32 in Wrapping`, `u8 in Saturating` or `u64 in Trapping`. These core
domains make the unsuffixed operators mean the suffixed ones, and the result
keeps the qualification. They suit values that are modular or clamped by
nature, such as a hash state or a sequence number. For one checked addition the
operator says the same thing without marking the value.

These policies do not all repair invalid operands: wrapping and saturation
never license integer division by zero. A value may carry a policy alongside
an independent meaning, such as `Km & Wrapping`, but not another arithmetic
policy; that pair has operators only where one is declared for exactly it.
A policy is added and removed with a cast, `as +Wrapping` and `as -Wrapping`,
like every other domain. Qualification changes future operations, not the
existing payload.

Bitwise operations preserve their integer carrier; Boolean `!` is distinct from
integer `~`. Exact shifts prove a nonnegative count below the value width.
Wrapping reduces the count modulo that width, Trapping checks it, and Saturating
keeps the Exact count obligation. Hardware masking is not an Exact proof.
The full [numeric rules](../spec/language/numeric_values.md#arithmetic-and-comparisons)
also distinguish value overflow from operand validity.

### Comparing different integer types

Integer comparisons compare mathematical values, even when the operands have
different widths or signedness:

```omega
machine before_end(cursor: i32, length: u64) -> bool {
    cursor < length
}

machine same_count(left: i32, right: u64) -> bool {
    left == right
}
```

`before_end(-1, 10)` is true. `same_count(10, 10)` is true. Neither comparison
converts the signed operand to unsigned, changes its value, or needs the other
operand to fit its type. The same rule covers `<`, `<=`, `>`, `>=`, `==` and `!=`.
It holds in proofs, compile-time evaluation, interpretation and native code.
A `u32` has the same range on every target; register width does not choose the
answer. See [integer equality and order](../spec/language/numeric_values.md#integer-equality-and-order).

This is not implicit arithmetic widening: `left + right` still needs an
applicable arithmetic operation and its representability proof or chosen policy.
Anonymous literals also keep their ordinary landing rules; a comparison does
not make an out-of-range literal inhabit an unsigned type. The builtin
comparisons take plain operands: a qualified operand uses a comparison its
domain declares or is cast first, as [Chapter 8](chapter_8_domains.md) shows.

For indexing, `cursor < length` is only half the obligation when the cursor is
signed: negative cursors satisfy it too. Establish `0 <= cursor` before access.

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

`as` never calls arbitrary user code, rounds, allocates, or silently chooses a
policy. Unit-scale conversion is an ordinary domain-library operation. See
[destination-owned conversion](../spec/language/numeric_values.md#destination-owned-conversion)
and [domain casts](../spec/language/domains.md#domain-casts).

### Where the wrap applies: at each node, at the declared width

Suppose `a: u32 in Wrapping` holds `0xFFFF_FFFE`. Its parent operations see that
wrapped 32-bit value, not an unbounded subtraction or a wider register image:

```omega
let shifted: u32 in Wrapping = a >> 1; // 0x7FFF_FFFF
let divided: u32 in Wrapping = a / 3; // 1431655764
let exact: u32 = divided as -Wrapping; // removes the policy; later math is Exact
```

Saturating and Trapping likewise act at each already-landed operation before
the parent consumes its result. A known Trapping overflow remains a runtime
trap. This does not let an anonymous initial value bypass landing: assigning
an out-of-range literal to a Saturating target is not a saturating conversion.

## Constants: two phases

An anonymous constant has an exact mathematical value but no selected machine
representation. `3.14` is exactly `157/50`. Landing chooses a type once: integer
landing checks integrality and range; floating landing rounds once.

After landing, all later folding obeys that type, format and domain. It cannot
turn a landed value back into an anonymous integer. A machine parameter is a
landing boundary even when the call is evaluated during compilation.

### Exact anonymous division and landing

Anonymous `/` is rational division, not truncating integer division:

```omega
let exact_integer: i32 = (7 / 2 * 2) as i32; // 7; explicit exact landing
let fractional: i32 = (7 / 2) as i32;        // error: 7/2 is not an integer
let typed_integer: i32 = 7i32 / 2 * 2;       // 6; typed integer division
let exact_float: f64 = 7 / 2 / 2;            // 1.75; floating landing
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

An apparent integer align-down can instead cancel an exact fraction. Omega
requires you to choose at this site:

```omega
let implicit: u32 = (4097 / 4096) * 4096;         // error: explicit landing required
let exact: u32 = ((4097 / 4096) * 4096) as u32;   // 4097; exact math, then land
let aligned: u32 = (4097u32 / 4096) * 4096;       // 4096; integer arithmetic
```

Cast the complete formula when the fractions are intentional. There is no need
to rearrange the formula to avoid fractional intermediates:

```omega
let duration_ms: u32 = ((1 / 3 + 1 / 6) * 1000) as u32; // 500
```

The final value must still be integral and in range. `(5 / 2) as u32` rejects;
`as` does not truncate. A type annotation alone is implicit landing, and decimal
spelling does not opt in. Typing an operand requests that carrier's arithmetic.

The cast covers only its operand. `(5 / 2) * 2 as u32` casts only `2` and
rejects when the other operand, `5/2`, must land as `u32`. Write
`((5 / 2) * 2) as u32` to get `5`. A cast outside a machine call cannot
acknowledge a fractional intermediate landing earlier at an argument.

The [diagnostic contract](../spec/language/numeric_values.md#fractional-intermediate-diagnostics)
retains the fractional origin through cancellation. This is an error, not a
suppressible warning. Already-typed arithmetic and floating landing keep their
existing rules.

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

## Float facts

`f32` and `f64` permanently mean binary32 and binary64. A target realizes
those meanings; it cannot redefine them. Finite arithmetic computes exact
meaning and rounds through the selected format, with separate special-value
rules for zeros, infinity and NaN.

### Require finite values

```omega
data Particle
requires
    alpha >= 0.0 && alpha <= 1.0,
{
    x: f64;
    speed: f64 in Finite;
    alpha: f32;
    mass: f64 in Finite & Positive;
}
```

Bare floats may contain NaN or infinity. `Finite` excludes them; the finite
bounds in `alpha`'s contract also imply `Finite`. These are invariant-window
facts, not runtime metadata or optimization permissions.

### Choose float overflow behavior

Quiet float semantics allow non-finite results. Trapping checks the semantic
result and traps on NaN or infinity. Saturating clamps finite-operand magnitude
overflow, but does not repair division by signed zero or invalid operations
such as `0.0f64 / 0.0`. Wrapping has no float candidate.

Finite inputs alone therefore do not justify this result:

```omega
// Rejected: the quotient could be infinite or NaN, so `+Finite` is unproved.
machine divide(a: f32 in Finite, b: f32 in Finite) -> f32 in Finite {
    ((a as -Finite) / (b as -Finite)) as +Finite
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

This selects an order that covers NaNs and signed zeros as well as ordinary
numbers. Arithmetic comparisons keep their IEEE meaning.

### Rounding

`a * b + c` rounds twice. A named fused multiply-add rounds once, so choose it
explicitly when that is the calculation you want. A `Finite` fact alone does
not permit rearranging arithmetic.

### Literals and compile-time arithmetic

`9.80665` denotes exactly `196133/20000` until landing. Anonymous arithmetic
rounds once into the destination float format; already-landed arithmetic uses
that format at each operation. Compile-time execution and runtime contracts
share `FloatSemantics`, not the host compiler's preferred floating type.

The [numeric reference](../spec/language/numeric_values.md) covers proof meaning,
NaN representations, and the floating control state required at foreign calls.

## Temporaries

```omega
View::render_line(&mut view, RoomFormatter::title(&room));
```

Temporary storage must outlive its derived borrows and receive ordinary
cleanup. Shared lending may be implicit for a shared-reference parameter;
mutable and write-only lending use explicit `&mut` and `&write`. Prefer a
named local when the lifetime or proof facts become difficult to see.

## Blocks

A block groups straight-line work and introduces local scope and cleanup edges:

```omega
{
    let mut room: Room;
    Lookup::find_room(&lookup, level, current_cell, &mut room);
    View::render_room(&mut view, &room);
}
```

It does not create a machine. `room` belongs to this block's work;
it need not become a field of the lookup, view, or surrounding context.

## Terminal expressions

A terminal value completes the current invocation:

```omega
machine shutdown() -> i32 {
    0
}
```

A call-shaped transition arm invokes a machine, nested or otherwise. Its eventual
result completes the invocation; its suspension markers and loan/cleanup checks
remain in force.

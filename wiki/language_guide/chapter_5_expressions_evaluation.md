# Chapter 5: Expressions and evaluation

This chapter covers calls, operators, indexing and arithmetic. The part most
unlike other languages is arithmetic: integer math must be shown not to
overflow, or you say what should happen when it does.

## Literals

```omega
let machine_value: i32 = 3i32;
let proof_value: Nat = 3nat;
let machine_float: f64 = 3.0f64;
```

A suffix fixes the type. Without one, a literal takes its type from where it
is used. `i32`, `u64`, `f32`, `f64` and friends are machine types; `Nat`, `Int`
and `Real` are mathematical types used in proofs.

## Evaluation order

Arguments and operands are evaluated left to right, once each. `&&` and `||`
short-circuit. Record fields are evaluated in the order the literal writes
them, not the order the type declares them.

An assignment picks its destination before it evaluates the right-hand side:

```omega
values[index] = replacement(&mut index);   // stores into the element chosen first
```

## Calls

```omega
let command: Command = Parser::resolve(&parser, &line);
View::render_room(&mut view, &room);
```

A dot selects a field. It never calls anything.

Calls that may wait carry a marker:

```omega
ordinary_call();                // neither
suspend may_park();             // may park this task
block may_block_worker();       // may block the worker thread
suspend block may_do_either();
```

A `suspend` call stands on its own: bind its result, then use it.

```omega
let next: u64 = suspend Source::next(&mut source);
let total: u64 = prefix + next;
```

## Operators

An operator is a machine declared with its token:

```omega
public machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 {
    add_vectors(left, right)
}

machine == Vec2::equals(left: &Vec2, right: &Vec2) -> bool {
    equal_vectors(left, right)
}
```

`a + b` on two `Vec2` values calls `Vec2::add`. The type's owner declares its
operators, so importing an unrelated package cannot change what `+` means.
Generic code gets an operator from a trait:

```omega
trait Ranked<T> {
    machine < compare(left: T, right: T) -> bool;
}
```

See [Chapter 14](chapter_14_traits.md#operator-requirements).

## Arrays, slices and indexing

```omega
let fixed: [Item; 4];
let view: &[Item] = ArrayAccess::as_slice(&fixed);
let first: Item = view[0];
let tail: &[Item] = view[1..];
```

`[T; N]` owns its elements and `&[T]` borrows a run of them. Every index needs
evidence that it is in bounds, usually from a `requires` or a branch:

```omega
machine read_at(items: &[u8], index: u64) -> u8
requires index < items.len
{
    items[index]
}
```

There is no runtime bounds check to fall back on: an index the compiler cannot
show is in range is rejected. Ranges are `a..b` (excludes `b`), `a..=b`
(includes it), `a..`, `..b` and `..`.

A mutable sub-slice is requested explicitly:

```omega
let middle: &mut [u8] = &mut view[2..4];
```

Your own types can be indexed by declaring `[]`:

```omega
machine [] at(buffer: &Buffer, index: u64) -> u8
requires index < 4
{
    buffer.bytes[index]
}
```

## Integer arithmetic

By default every operation must be shown to fit its type. There is no
implicit widening and no silent wraparound:

```omega
machine total(count: u32, size: u32) -> u32
requires count <= 1000
requires size <= 1000
{
    count * size        // fits: at most 1,000,000
}
```

When you want overflow to do something specific, say so on the operator:

```omega
let next: u64 = cursor +! width;   // trap on overflow
let mixed: u32 = hash *% 31;       // wrap
let raised: u8 = level +| step;    // clamp to 0..=255
```

| Suffix | On overflow |
| --- | --- |
| `!` | Checks at run time and traps. |
| `%` | Wraps modulo the type's width. |
| `\|` | Clamps to the type's minimum or maximum. |

The suffixes work on `+`, `-`, `*`, `/`, `%`, `<<` and `>>`. Wrapping and
clamping change what overflow does; they do not make division by zero legal.

If a value should always use one policy, put it in the type:

```omega
let state: u32 in Wrapping = seed as +Wrapping;
let next: u32 in Wrapping = state * state;        // wraps, no suffix needed
let plain: u32 = next as -Wrapping;
```

`Wrapping`, `Saturating` and `Trapping` are [domains](chapter_8_domains.md).

## Comparing integers

Comparisons compare the numbers, whatever their types:

```omega
machine before_end(cursor: i32, length: u64) -> bool {
    cursor < length      // before_end(-1, 10) is true
}
```

Nothing is converted. For indexing, remember that a negative signed cursor is
also "less than the length", so establish `0 <= cursor` too.

## Unsuffixed constants are exact

A literal with no type yet is an exact number, and `/` on such numbers is
exact division, not integer division:

```omega
let typed: i32 = 7i32 / 2 * 2;            // 6: integer division
let exact: i32 = (7 / 2 * 2) as i32;      // 7: exact math, then convert
let bad: i32 = (7 / 2) as i32;            // Rejected: 3.5 is not an integer.
let duration_ms: u32 = ((1 / 3 + 1 / 6) * 1000) as u32;   // 500
```

If a formula passes through a fraction on its way to an integer, the compiler
makes you choose: give an operand a type to get integer arithmetic, or cast
the whole formula with `as` to keep the exact math. `as` never truncates or
rounds.

Integer `/` and `%` truncate toward zero: `-7 / 2` is `-3` and `-7 % 2` is
`-1`.

## Converting between numbers

Conversions that can lose information are named for their destination, and
the result type picks the behaviour:

```omega
let exact: i32 = I32::from_f64(value);                  // must be shown in range
let checked: i32 in Trapping = I32::from_f64(value);    // traps if not
let clamped: i32 in Saturating = I32::from_f64(value);  // clamps; NaN gives 0
```

## Floats

`f32` and `f64` are IEEE binary32 and binary64 on every target. A plain float
may be NaN or infinite; `Finite` rules that out:

```omega
data Particle {
    x: f64;
    speed: f64 in Finite;
    mass: f64 in Finite & Positive;
}
```

Comparisons follow IEEE: anything compared with NaN is false, except `!=`.
Sorting needs a total order and asks for one by name:

```omega
sort_by<F64::TotalOrder>(&mut samples);
```

Finite inputs do not make a result finite: a division can still overflow or
divide by zero, and the compiler asks you to rule those out.

## Blocks and temporaries

```omega
{
    let mut room: Room;
    Lookup::find_room(&lookup, level, current_cell, &mut room);
    View::render_room(&mut view, &room);
}
```

A block scopes its locals; `room` is cleaned up at the closing brace.

## Where next

[Chapter 6](chapter_6_pattern_matching_dispatch.md) covers `match`. The
complete rules are in [expressions](../spec/language/expressions.md) and
[numeric values](../spec/language/numeric_values.md), which also covers
rounding, NaN payloads and the standard math helpers.

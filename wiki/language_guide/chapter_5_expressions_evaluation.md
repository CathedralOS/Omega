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
let next: usize = index + 1;
let weak: bool = self.health <= 25;
```

The same operator spelling may exist for machine numbers and proof numbers. The
operand types decide which rules and obligations apply.

Operators should be understood as shorthand for resolved semantic operations,
not as syntax with a completely separate meaning model. In that sense,
`left + right` is conceptually like a call to the appropriate add/concat
operation for the operand meaning in scope.

This model also applies to privileged syntax. `items[index]` should be
understood as an indexing operator, not as raw pointer syntax. `items[1..]`
should be understood as a range-slice operator. Those operators should have a
semantic home that users and tools can inspect, while their trusted primitive
implementation is bound through the compiler/runtime layer.

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
- `Str`: owned string/text storage.
- `StrView`: borrowed string/text view.

The exact surface spelling may stay Rust-like for a while:

```omega
let fixed: [Item; 4];
let view: &[Item] = fixed.as_slice();
let text: String;
```

But semantically, `Array`, `Vec`, and `Slice` should be visible core concepts,
not just implicit compiler behavior. `Array` and `Vec` are owners. `Slice` is
the common borrowed view they can produce. Likewise, an owned string can produce
a string view.

The implementation can still use privileged internal carriers. A slice view is
likely lowered as a descriptor such as pointer plus length. A vector is likely
lowered as an owned buffer with pointer, length, and capacity. Those carriers
belong near the trusted/primitive layer, while the public proof and operator
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
- Text views expose byte-oriented operations such as `StrView::byte` and
  `StrView::range`; character or grapheme indexing must be a separate semantic
  operation because UTF-8 byte positions are not the same as user-visible
  characters.

Omega loops often look like repeated transitions over either:

- a bounded index carried in state parameters, or
- a shrinking slice window where `[0]` remains valid until the window is empty.

The important point is that indexing is not magical pointer syntax. It is a
normal operation guarded by proof of a valid range. For built-in core
collections, the operator contract and named measures should be visible as part
of the core language surface; the low-level pointer/descriptor work belongs to a
trusted primitive implementation layer below that surface.

The visible core declaration should therefore look like a normal contract plus a
named trust root:

```omega
operator Array::index<T>(items: &Array<T>, index: usize) -> T
requires
    index < items.len
trust compiler_array_index;

operator Vec::index<T>(items: &Vec<T>, index: usize) -> T
requires
    index < items.len
trust compiler_vec_index;

operator Slice::index_mut<T>(items: &mut [T], index: usize) -> &mut T
requires
    index < items.len
trust compiler_slice_index_mut;

operator Slice::range_mut<T>(items: &mut [T], start: usize, end: usize) -> &mut [T]
requires
    start <= end && end <= items.len
trust compiler_slice_range_mut;

operator Slice::from<T>(items: &[T], start: usize) -> &[T]
requires
    start <= items.len
trust compiler_slice_from;

operator StrView::byte(text: &str, index: usize) -> u8
requires
    index < text.len
trust compiler_str_view_byte;

operator StrView::range(text: &str, start: usize, end: usize) -> &str
requires
    start <= end && end <= text.len
trust compiler_str_view_range;

operator Vec::with_capacity<T>(capacity: usize) -> Vec<T>
trust compiler_vec_allocate;

operator String::with_capacity(capacity: usize) -> String
trust compiler_string_allocate;
```

The proof checker owns `start <= items.len`. The trusted primitive owns the
descriptor/pointer rewrite that actually constructs the narrower view.
For allocation-facing contracts such as `Vec::with_capacity` and
`String::with_capacity`, the public core
declaration owns the source meaning while the trusted primitive owns allocator
and buffer initialization details.

Operator declarations form overload sets by call signature. The call signature
is the operator path plus parameter types; return type alone does not create a
distinct overload. Generic signatures are compared by structure, not by the
spelling or declaration order of type parameters, so these two declarations
describe the same candidate and must be rejected as a duplicate:

```omega
operator Slice::index<T>(items: &[T], index: usize) -> T;
operator Slice::index<U>(items: &[U], index: usize) -> U;
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

Machine integers also need an explicit arithmetic policy somewhere in the
language model.

Possible policies:

- `exact`: prove overflow, underflow, division by zero, invalid shifts, and
  similar hazards cannot happen.
- `wrapping`: arithmetic wraps according to the fixed-width representation.
- `trap`: runtime trap on arithmetic failure.
- `saturating`: arithmetic clamps to the representable minimum or maximum.
- `checked`: operations that can fail must surface failure explicitly.

The likely default is exact/proven arithmetic. Weaker behavior should be
explicit because it changes both proof obligations and runtime behavior.

## Float Facts

Float constraints describe correctness facts, not optimization permissions.

```omega
data Motion {
    speed: f32;
}
```

Working interpretation:

- `finite` means the value is not `NaN`, `+inf`, or `-inf`.
- `0.0f..=100000.0f` is the intended range spelling for a float fact.
- Float constraints are not runtime metadata.
- Float constraints do not automatically permit reassociation, signed-zero
  erasure, reciprocal transforms, or other fast-math rewrites.

The language probably needs two separate layers:

- semantic constraints: facts that must be true, such as `finite`,
  `non_negative`, or `a..=b`
- optimization permissions: facts about which rewrites are acceptable

For now, this chapter only covers semantic constraints.

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

# Chapter 6: Bounded Types

Omega should allow data types to carry proof-friendly refinements.

```omega
fn clamp(
    value: i32,
    min: const i32,
    max: const i32
) -> i32[range<min, max>] {
}
```

Owned data may carry invariants:

```omega
owns mass: i32[range<1, 100>];
```

Working interpretation:

- `i32[range<1, 100>]` is an `i32` refined by a range invariant.
- `i32[range<min, max>]` is an `i32` refined by compile-time or proof-visible bounds.
- Ranges are assumed inclusive for now unless the syntax grows explicit open/closed bounds.
- `const` parameters may be used in type-level constraints.
- The compiler emits proof obligations anywhere a value is assigned, produced, or transitioned into a bounded slot.
- Invariants are compile-time proof facts, not RTTI.

The clamp graph produces an obvious proof shape:

```omega
fn clamp(value: f32, min: f32, max: f32) -> f32 {
    match (value < min, value > max) {
        (true, _) -> min
        (false, true) -> max
        (false, false) -> value
    }
}
```

The match arms create a proof partition:

- In the `(true, _)` arm, the compiler knows `value < min`.
- In the `(false, true)` arm, the compiler knows `value >= min` and `value > max`.
- In the `(false, false)` arm, the compiler knows `value >= min` and `value <= max`.

When a boolean or tuple match becomes hard to read, name the facts before
matching. The facts become part of the proof context for each arm.

## Generic Invariants

Bounds may be generic over compile-time or proof-known values.

```omega
fn clamp(
    value: i32,
    min: const i32,
    max: const i32
) -> i32[range<min, max>] {
}
```

This means each instantiation of `clamp` carries a proof obligation for the specific `min` and `max` in scope.

Important runtime rule:

- `i32[range<min, max>]` is still represented as an `i32`.
- The range is not runtime type information.
- Code cannot inspect the invariant at runtime as metadata.
- If the compiler cannot prove the invariant, it should emit a diagnostic rather than silently inserting hidden runtime checks.
- Debug builds or proof artifacts may choose to emit extra validation, but that is instrumentation, not the language's core runtime model.

This keeps invariants as part of the compiler's reasoning system. They describe what must be true, not an object header or dynamic type tag.

## Slices And Invariant Parameters

Owning growable containers and stable indexed views should be distinct.

```omega
data Inventory {
    items: Vec<InventoryItem>;
}

fn find_item(inventory: &Inventory, kind: ItemKind) -> Option<usize> {
    let items: &[InventoryItem] = inventory.items.as_slice();

    match items.len > 0 {
        true -> find_item_at(items, kind, 0)
        false -> None
    }
}

fn find_item_at(
    items: &[InventoryItem, [non_empty]],
    kind: ItemKind,
    base_index: usize,
) -> Option<usize> {
    let found: bool = items[0].kind == kind;
    let rest: &[InventoryItem] = items[1..];
    let next_index: usize = base_index + 1;

    match found {
        true -> Some(base_index)
        false -> match rest.len > 0 {
            true -> find_item_at(rest, kind, next_index)
            false -> None
        }
    }
}
```

Working interpretation:

- `Vec<T>` owns growable storage.
- Fields inside `data` are owned by default. Omega does not need an `owned`
  keyword for ordinary stored fields.
- `&[T]` is built-in borrowed slice-view syntax, intentionally close to Rust.
- `&mut [T]` is the corresponding built-in unique mutable slice-view syntax.
- `Vec<T>.as_slice()` creates a stable immutable view for the borrow lifetime.
- `Vec<T>.as_mut_slice()` creates a stable mutable view for the borrow lifetime.
- Slice views expose proof-visible facts such as `len`.
- Slice views may carry type-scoped invariant parameters, such as
  `&[T, [non_empty]]`.
- `items[index]` is a proof-requiring operator. The compiler must prove
  `index < items.len` from the current facts.
- `items[0]` is valid for `&[T, [non_empty]]` because that invariant means
  `items.len > 0` for slice views.
- Slice ranges such as `items[1..]` create new slice views with updated bounds.
- Slice indexing should not silently add hidden runtime bounds checks as the
  language model. Debug or proof-instrumented builds may add checks as
  instrumentation.

Invariant names are resolved in the namespace of the type being instantiated.
For slice views, `non_empty` means the slice-specific fact `len > 0`. Another
type may export a different invariant with the same name if that name is scoped
to that type.

`Vec<T>` belongs in the allocation/runtime layer, while slice views are core
borrowed language concepts. The public language model does not need to expose a
raw pointer field for `Vec<T>`; a vector owns storage, length, and capacity, and
its methods manufacture borrowed slice views with proof-visible guarantees.
Mutation through `Vec<T>` may invalidate existing slice views; the borrow system
must enforce that an active `&[T]` view cannot be invalidated by a conflicting
mutable operation.

## Invariant Aliases

Repeated constraint lists may be named for reuse.

```omega
invariant finite_value = [finite];
invariant speed_range = [finite_value, range<0.0f, 100000.0f>];

machine main {
    owns speed: f32[speed_range] = 0.0f;
}
```

Working interpretation:

- Invariant names use normal Rust-style value naming: `speed_range`, not `SpeedRange`.
- An invariant alias expands at compile time into its constraint list.
- Aliases may compose by referring to earlier or later aliases.
- Aliases do not create runtime types, wrappers, tags, RTTI, or hidden storage.
- A bounded type like `f32[speed_range]` is still represented as an `f32`.
- Recursive aliases are invalid because they never produce a concrete proof fact.

## Proof Numbers And Machine Numbers

Omega should distinguish mathematical numbers from machine representations.

Working categories:

- `UInt` is a proof-level natural number: zero or positive, unbounded in the mathematical model.
- `Int` is a proof-level integer: unbounded in the mathematical model.
- `Real` is a proof-level real number: useful for specifications, generic numeric contracts, and reasoning about approximation.
- `i32`, `u64`, `f32`, and similar types are machine representations with finite storage and target-level behavior.

This lets Omega write proof-facing APIs without pretending every value is already a machine value:

```omega
fn clamp(value: Real, min: Real, max: Real) -> Real[range<min, max>];
```

A machine implementation may call or instantiate that shape only when it can prove the machine values satisfy the required embedding or approximation rule.

For example, an `f32` value might be usable where a `Real` contract is expected only if the compiler has facts such as:

```omega
f32[finite, approx<Real, eps=1e-12>]
```

Working interpretation:

- `Real` is not a runtime floating-point type by default.
- `UInt`, `Int`, and `Real` are proof/spec types first.
- Machine values may carry evidence that they embed into, approximate, or preserve facts about proof numbers.
- Lowering to native code must erase proof-only numbers or replace them with proven machine representations.

## Integer Arithmetic Semantics

Machine integers should carry explicit arithmetic semantics.

The likely default should be exact arithmetic:

```omega
owns health: i32[exact] = 100;
```

`i32[exact]` means operations on the value must be proven not to overflow, underflow, divide by zero, or otherwise leave the defined machine integer domain. This is the proof-heavy default: if the compiler cannot prove the operation is safe, it emits a diagnostic or requires a different arithmetic mode.

Possible modes:

- `i32[exact]`: compile-time proof required for overflow, underflow, division by zero, invalid shift, and similar arithmetic hazards.
- `i32[wrapping]`: arithmetic wraps according to the fixed-width representation.
- `i32[trap]`: runtime trap on arithmetic failure.
- `i32[saturating]`: arithmetic clamps to the representable minimum or maximum.
- `i32[checked]`: operations that can fail must surface failure through an explicit result shape rather than silently continuing.

The exact spelling is still provisional, but the semantic split is important. Overflow policy is not merely an optimization detail; it changes proof obligations and runtime behavior.

Arithmetic modes compose with ordinary refinements:

```omega
owns ammo: i32[exact, range<0, 999>] = 30;
owns tick_counter: u64[wrapping] = 0;
```

Working interpretation:

- If no mode is written, machine integers should probably default to `exact`.
- Weaker modes must be explicit because they discard proof strength.
- `checked` likely changes operator typing because failure must be represented.
- `trap` is checked at runtime, so the build artifact should list the runtime obligation.

## Float Invariants

Float types need refinements too, but they are trickier than integers.

Omega should assume IEEE semantics by default, then allow extra proof information to be layered on top:

```omega
owns speed: f32[finite, range<0.0f, 100000.0f>];
```

Working interpretation:

- `f32` and `f64` are IEEE floats by default.
- `finite` means the value is not `NaN`, `+inf`, or `-inf`.
- `range<0.0f, 100000.0f>` is a proof refinement over the numeric value.
- Float refinements are compile-time proof facts, not runtime metadata.
- Float ranges should not imply algebraic rewrite permissions.

The last point matters. "This value is finite and in range" is not the same thing as "addition is associative" or "the compiler can ignore signed zero." Floating point has several different concerns that should not be collapsed into one flag.

Omega likely needs two separate layers:

- Semantic invariants: facts that must be true, such as `finite`, `non_nan`, `non_negative`, or `range<a, b>`.
- Optimization permissions: facts about which rewrites are acceptable, such as reassociation, reciprocal transforms, approximate math, or ignoring signed zero.

Some float optimizations are naturally onion-like: each permission expands the set of legal rewrites. Others are more domain-specific: a program may accept approximate square roots but still care about commutative behavior or signed zero.

For now, bounded float syntax should describe correctness facts only. Optimization policy needs its own syntax or mode later.

Proof-level `Real` gives Omega a clean way to specify ideal numeric behavior without lying about `f32`.

```omega
fn ideal_distance(a: Real, b: Real) -> Real;
fn fast_distance(a: f32[finite], b: f32[finite]) -> f32[finite, approx<Real, eps=1e-5>];
```

Working interpretation:

- `Real` describes the mathematical contract.
- `f32[finite]` describes representable runtime data.
- `approx<Real, eps=...>` describes the relationship between the runtime value and the proof/spec value.
- Approximation facts are invariants/proof facts, not permission to perform arbitrary fast-math rewrites.

## Rust Comparison

Rust has related pieces, but not this exact feature.

- `1..100` and `1..=100` are range values.
- Range patterns can match values in some contexts.
- Const generics can parameterize types over compile-time values.
- Newtypes and smart constructors can enforce ranges by convention.

None of those make `i32[range<min, max>]` a native, compiler-proved primitive refinement on stable Rust. Omega's range syntax is therefore its own proof-facing type annotation, not a Rust compatibility feature.

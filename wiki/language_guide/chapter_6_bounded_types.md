# Chapter 6: Bounded Types

Omega should allow data types to carry proof-friendly refinements.

```omega
state clamp(
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
state Clamp(&mut self, value: f32, min: f32, max: f32) -> f32 {
    -> self.ClampLow(min) when value < min;
    -> self.ClampHigh(max) when value > max;
    -> self.ClampDone(value);
}
```

The ordered transitions create a proof partition:

- If `value < min`, `ClampLow(min)` produces `min`.
- If `value > max`, `ClampHigh(max)` produces `max`.
- Otherwise, `ClampDone(value)` is only reachable when `min <= value <= max`.

The order matters. A later transition inherits the fact that earlier transitions did not fire.

## Generic Invariants

Bounds may be generic over compile-time or proof-known values.

```omega
state clamp(
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

## Rust Comparison

Rust has related pieces, but not this exact feature.

- `1..100` and `1..=100` are range values.
- Range patterns can match values in some contexts.
- Const generics can parameterize types over compile-time values.
- Newtypes and smart constructors can enforce ranges by convention.

None of those make `i32[range<min, max>]` a native, compiler-proved primitive refinement on stable Rust. Omega's range syntax is therefore its own proof-facing type annotation, not a Rust compatibility feature.

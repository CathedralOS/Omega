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
state clamp(&mut self, value: f32, min: f32, max: f32) -> f32 {
    -> self.clamp_low(min) when value < min;
    -> self.clamp_high(max) when value > max;
    -> self.clamp_done(value);
}
```

The ordered transitions create a proof partition:

- If `value < min`, `clamp_low(min)` produces `min`.
- If `value > max`, `clamp_high(max)` produces `max`.
- Otherwise, `clamp_done(value)` is only reachable when `min <= value <= max`.

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

## Rust Comparison

Rust has related pieces, but not this exact feature.

- `1..100` and `1..=100` are range values.
- Range patterns can match values in some contexts.
- Const generics can parameterize types over compile-time values.
- Newtypes and smart constructors can enforce ranges by convention.

None of those make `i32[range<min, max>]` a native, compiler-proved primitive refinement on stable Rust. Omega's range syntax is therefore its own proof-facing type annotation, not a Rust compatibility feature.

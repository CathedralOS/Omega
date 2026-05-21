# Chapter 7: Types, Constraints, And Invariants

Value constraints are proof facts attached to ordinary values.

They are not a separate runtime object model. A constrained `i32` is still
represented as an `i32`; the extra syntax describes what the compiler must
prove when the value is created, assigned, passed, or used at a boundary.

```omega
data Body {
    mass: i32[range<1, 100>];
}
```

Working interpretation:

- `i32[range<1, 100>]` is an `i32` with a range invariant.
- Ranges are assumed inclusive until the syntax grows explicit open/closed
  bounds.
- Invariants are compile-time proof facts, not RTTI.
- If the compiler cannot prove a constraint, the normal result is a diagnostic.
- Debug or proof builds may emit validation, but validation is instrumentation,
  not the core semantics.

## Invariant Propagation

Invariants flow through assignments, calls, and transitions as proof facts.

```omega
data Player {
    health: i32[range<1, 100>];
}

machine Player::take_damage(
    &mut self,
    amount: i32[range<1, 100>]
) {
    relax self.health {
        self.health -= amount;
        Player::restore_health_range(&mut relaxed self.health);
    }

    transition self.health <= 25 {
        true -> bloodied()
        false -> still_alive()
    }

    state bloodied(&mut self) {
    }

    state still_alive(&mut self) {
    }
}
```

The useful idea is not that `relax` means "anything goes." It means the
compiler has a proof debt. The normal invariant must be restored before control
can leave the relax scope.

## Generic Constraints

Bounds may refer to compile-time or proof-visible values.

```omega
machine Math::clamp_i32(
    value: i32,
    min: const i32,
    max: const i32,
    out: &mut i32[range<min, max>]
) {
    match (value < min, value > max) {
        (true, _) -> {
            out = min;
        }
        (false, true) -> {
            out = max;
        }
        (false, false) -> {
            out = value;
        }
    }
}
```

The match partitions create facts:

- In the `(true, _)` arm, the compiler knows `value < min`.
- In the `(false, true)` arm, the compiler knows `value >= min` and
  `value > max`.
- In the `(false, false)` arm, the compiler knows `value >= min` and
  `value <= max`.

Those facts are what let the compiler discharge the assignment into
`out: i32[range<min, max>]`.

## Invariant Aliases

Repeated constraint lists may be named for reuse.

```omega
invariant finite_value = [finite];
invariant speed_range = [finite_value, range<0.0f, 100000.0f>];

data Motion {
    speed: f32[speed_range];
}
```

Working interpretation:

- Alias names use ordinary value naming, such as `speed_range`.
- An alias expands at compile time into its constraint list.
- Aliases do not create runtime wrappers, tags, RTTI, or hidden storage.
- Recursive aliases are invalid because they never produce concrete proof facts.

## Slices

Owning growable containers and stable indexed views should be distinct.

```omega
data Inventory {
    items: Vec<InventoryItem>;
}

machine Inventory::first_item(
    &self,
    out: &mut Option<InventoryItem>
) {
    let items: &[InventoryItem] = self.items.as_slice();

    match items.len > 0 {
        true -> {
            out = Some(items[0]);
        }
        false -> {
            out = None;
        }
    }
}
```

Working interpretation:

- `Vec<T>` owns growable storage.
- `&[T]` is a borrowed slice view.
- `&mut [T]` is a unique mutable slice view.
- Slice views expose proof-visible facts such as `len`.
- `items[index]` requires proof that `index < items.len`.
- `items[0]` is valid in the `true` arm because the match fact proves
  `items.len > 0`.
- Slice ranges such as `items[1..]` create new slice views with updated facts.

## Arithmetic Modes

Machine integers should carry explicit arithmetic semantics.

```omega
data Player {
    health: i32[exact, range<0, 100>];
    ammo: i32[exact, range<0, 999>];
    tick_counter: u64[wrapping];
}
```

Possible modes:

- `exact`: prove overflow, underflow, division by zero, invalid shifts, and
  similar hazards cannot happen.
- `wrapping`: arithmetic wraps according to the fixed-width representation.
- `trap`: runtime trap on arithmetic failure.
- `saturating`: arithmetic clamps to the representable minimum or maximum.
- `checked`: operations that can fail must surface failure explicitly.

The likely default is `exact`. Weaker behavior should be explicit because it
changes both proof obligations and runtime behavior.

## Proof Numbers And Machine Numbers

The language should distinguish mathematical numbers from machine
representations.

Working categories:

- `UInt` is a proof-level natural number.
- `Int` is a proof-level integer.
- `Real` is a proof-level real number.
- `i32`, `u64`, `f32`, and similar types are concrete machine representations.

Proof-level numbers are useful for specifications, constraints, and generic
numeric reasoning. Lowering to native code must erase proof-only numbers or
replace them with proven machine representations.

## Float Constraints

Float constraints describe correctness facts, not optimization permissions.

```omega
data Motion {
    speed: f32[finite, range<0.0f, 100000.0f>];
}
```

Working interpretation:

- `finite` means the value is not `NaN`, `+inf`, or `-inf`.
- `range<0.0f, 100000.0f>` is a proof refinement over the numeric value.
- Float constraints are not runtime metadata.
- Float constraints do not automatically permit reassociation, signed-zero
  erasure, reciprocal transforms, or other fast-math rewrites.

The language probably needs two separate layers:

- Semantic constraints: facts that must be true, such as `finite`,
  `non_negative`, or `range<a, b>`.
- Optimization permissions: facts about which rewrites are acceptable.

For now, this chapter only covers semantic constraints.

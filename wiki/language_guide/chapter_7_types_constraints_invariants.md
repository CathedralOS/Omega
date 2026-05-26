# Chapter 7: Types, Contracts, And Invariants

Omega no longer puts invariant syntax directly on types with `Type[...]`.

Proof obligations live in contracts, domains, and local flow facts. Values are
still stored as ordinary machine types; the compiler is responsible for proving
the facts that APIs and mutations require.

```omega
data Body {
    mass: i32;
}

machine Body::set_mass(&mut self, mass: i32)
    requires mass in 1..=100
    ensures self.mass in 1..=100
{
    self.mass = mass;
}
```

Working interpretation:

- `mass: i32` stays plain type information.
- Contracts carry the proof surface.
- Rust-style ranges such as `1..10` and `1..=10` are the intended interval
  syntax in contracts and invariant facts.
- Invariants are compile-time proof facts, not RTTI.
- If the compiler cannot prove a constraint, the normal result is a diagnostic.
- Debug or proof builds may emit validation, but validation is instrumentation,
  not the core semantics.

## Invariant Propagation

Invariants flow through assignments, calls, and transitions as proof facts.

```omega
data Player {
    health: i32;
}

machine Player::take_damage(
    &mut self,
    amount: i32
) ensures self.health in 0..=100 {
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

## Generic Contracts

Bounds may refer to compile-time or proof-visible values.

```omega
machine Math::clamp_i32(
    value: i32,
    min: const i32,
    max: const i32,
    out: &mut i32
) requires min <= max
  ensures out in min..=max
{
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

Those facts are what let the compiler discharge the postcondition
`out in min..=max`.

## Named Proof Vocabulary

Repeated proof conditions may still want names, but not as `Type[...]` sugar.
The likely homes are:

- domains for semantic states
- helper machines that establish a fact
- reusable proof or contract aliases once that surface is designed explicitly

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

Machine integers still need explicit arithmetic semantics, but not as bracketed
type suffixes.

```omega
data Player {
    health: i32;
    ammo: i32;
    tick_counter: u64;
}
```

Possible modes or policies:

- `exact`: prove overflow, underflow, division by zero, invalid shifts, and
  similar hazards cannot happen.
- `wrapping`: arithmetic wraps according to the fixed-width representation.
- `trap`: runtime trap on arithmetic failure.
- `saturating`: arithmetic clamps to the representable minimum or maximum.
- `checked`: operations that can fail must surface failure explicitly.

The likely default is exact/proven arithmetic. Weaker behavior should be
explicit because it
changes both proof obligations and runtime behavior.

The key design goal is that arithmetic behavior must be explicit somewhere, but
not as permanent runtime type information. Scoped policy, operator variants,
and domain-sensitive operator resolution are all better candidates than a type
suffix.

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

- Semantic constraints: facts that must be true, such as `finite`,
  `non_negative`, or `a..=b`.
- Optimization permissions: facts about which rewrites are acceptable.

For now, this chapter only covers semantic constraints.

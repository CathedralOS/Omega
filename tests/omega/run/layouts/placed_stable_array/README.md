# Indexed Stable placement

This is a Cathedral-driven repair under **PLAN-LAID-VIEWS**, not closure of
that capability or a completed nine-place fact row. One field permission covers
a fixed scalar-array storage region. A bounded index selects a scalar accessor;
it does not introduce another permission or turn an accessor into an lvalue.

`main.omg` reads and writes both elements across helper calls and then checks
the original backing. The sibling `placed_stable_nested_array` fixture uses
two dynamic selectors below a nonzero field offset, preserving a sentinel and
neighboring elements. Both require actual native and Terminal exit 0:

```text
omega run --both tests/omega/run/layouts/placed_stable_array/main.omg
omega run --both tests/omega/run/layouts/placed_stable_nested_array/main.omg
```

Reject controls under `tests/omega/fail/layouts/placed_stable_array_*` remove
the index bound, supply only a shared borrow, or remove write permission.
Each must reject its own obligation, not fail package setup.

The sibling `placed_stable_distinct_fields` keeps a read-only scalar, a read-only
array and a writable array distinct in one plan. Its reads, writes and original
backing checks exercise the separate operation identities.

`pass/layouts/resident_array_field_access` is an embedding entry with explicit
host-admitted vacant backing, not a hosted ProgramEntry. Its selected supply
keeps two adjacent intervals distinct through normalization; the second scalar
straddles their boundary. The extra external-read capability on the first
interval is not consumed by the Stable operation.

Temporary artifact-corruption and embedding diagnostics are retained with the
qualification evidence, not as Rust compiler behavior tests. They corroborate
rejection of shared operation leaves, refusal to publish a reference with a
missing initialized RAM cell, and actual resident bytes across the supply
boundary. The source fixtures and Cathedral are the durable execution controls.

This repair covers contiguous `At` placement of fixed scalar-array trees.
It does not establish arbitrary strided layouts, aggregate whole-value access
through a scalar primitive, External/Atomic aggregate access, or a new source
of backing authority. Cathedral's separate QEMU oracle observes actual indexed
resident-page writes and whole-page extraction after firmware exit.

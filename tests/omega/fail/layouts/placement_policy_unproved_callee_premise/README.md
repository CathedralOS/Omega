# Policy call-site premises

`CheckedPolicy::plan` calls its helper with 8 despite `requires value <= 7`.
The resulting layout would otherwise fit its one `u64` field. The compiler must
refuse the demanded invocation before evaluating that policy; projecting away
unrelated runtime declarations does not grant a callee premise.

The positive customer is the unchanged
`pass/layouts/placed_view_source_access_exit`: its policy calls
`AccessPlan::with`, whose private finite search uses ordinary preconditions.
The early policy probe contains four machines; the later input also contains
runtime accessors awaiting their authoritative placement plans. Those unrelated
accessors cannot be obligations of the policy invocation. The shared dependency
projection retains the policy's actual body, type, proof and operator closure,
including every helper contract, for ordinary checking.

Run the source controls with:

```sh
python3 tools/corpus_gate.py --filter layouts/placed_view_source_access_exit,layouts/access_plan_inaccessible_seed,layouts/placement_policy_unproved_callee_premise
```

Hosted entry policy execution remains constrained by the unchanged
`pass/filesystem/native_crud` and `pass/time/runtime_duration_core_exit`.
The former must print its authored PASS line natively. Its Terminal foreign
filesystem binding has no trusted host implementation; native success is not
Terminal host acceptance. The latter must exit 70 in both engines.

## Library declaration inventory

At `c1303f89d4`, `core/layout.omg` has these nine parameter suffixes; they
remain unchanged through the migration base `e7929ee7fae`:

| Position | Original bound | Replacement premise |
| --- | --- | --- |
| `with_bounded.fuel`, `seek.fuel`, `inspect.fuel`, `advance.fuel` | `u64 [1..=128]` | `fuel >= 1 && fuel <= 128` |
| `seek.index` | `u64 [0..=32]` | `index <= 32` |
| `inspect.index`, `replace.index`, `advance.index` | `u64 [0..=31]` | `index <= 31` |
| `grow_invalid.field_count` | `u64 [0..=31]` | `field_count <= 31` |

`std/targets/macos_x86_64/entry.omg` also has
`field_range.first_field: u64 [0..=254]` at that inventory revision. Its shared
successor in `std/entry_shapes.omg`, absent at `c1303f89d4`, has the same suffix
at `e7929ee7fae`. Both now use `first_field: u64` with
`requires first_field <= 254`. The unsigned carrier preserves each original
zero lower bound. The machine/state arrival contracts retain every upper and
nonzero bound; no interval-only nominal domain or maximum-sized allocation
replaces them. This inventory covers these eleven parameters, not the complete
scalar-contract migration.

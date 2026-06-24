# `String` retirement — execution recipe (#66 Phase B2–B4)

> **Status: READY — verified 2026-06-22, not yet executed.** This is an **atomic**
> change (no green-preserving incremental step): the tree is red from the keystone
> edit until the last site + corpus file is migrated, then `cargo test --workspace`
> comes back green. Run it as one focused push (worktree or the dedicated branch;
> `abc38382` is the last green commit). Everything below was traced against the
> real code so the push is mechanical, not exploratory.

## Why it's atomic

The keystone (string literals satisfying `PrimitiveType::String`) cascades to every
`: string` / `String` declaration the moment it changes, and **all 15 compiler
sites are live** — `String` is *implemented as* the fat-slice/text descriptor
(`size = pointer*2`, `fat_descriptor_layout`, `text_descriptor`), and the
`&[u8] in Utf8` path was already added *beside* it (e.g. `guards.rs`:
"recognize such a slice-descriptor place **not** `PrimitiveType::String` too").
So there is no dead branch to delete incrementally; the variant and its uses come
out together.

## Allocator: NOT a blocker (verified)

The allocator is still stage-1 design, and `Vec<u8> in Utf8` (owned/growable heap
text) is genuinely gated on it — **but that is orthogonal to this retirement**:

- The owned surface (`with_capacity`, `push_str`, `Capacity`, `from_utf8`, …) is
  declared in `omega/language/core/str.omg` as **`boundary operator`s** — host/
  runtime-provided, *not natively lowered today*. There is no native owned-string
  behavior to regress.
- The bulk — literals, borrowed `&string`, bounded `[u8;N]` / `FixedVec<u8>`
  (ships today) — is **not** gated.
- Retiring `String` routes owned text onto `Vec<u8> in Utf8` (the same already-gated
  boundary path) and borrowed text onto `&[u8] in Utf8`. **Net regression: zero.**
- The dungeon and samples use **no** owned-growable strings, so they migrate clean.

## The model (target)

`string`/`String` stop being magic builtins; text becomes `{carrier} in Utf8`:

| Today | After |
| --- | --- |
| string literal `"..."` (typed owned `String`) | static `&[u8] in Utf8` view |
| `&string` borrowed window | `&[u8] in Utf8` |
| owned `String` (capacity, `push_str`) | `Vec<u8> in Utf8` (boundary-gated, unchanged status) or `FixedVec<u8> in Utf8` (bounded, works now) |

Borrowed operators rewrite: `Str::Length`→`.len`, `Str::bytes`→the slice itself,
`Str::byte(t,i)`→`t[i]`, `Str::range(t,a,b)`→`t[a..b]`. The `Utf8`/`NoNul` domain +
the `from_utf8`/`validate_*` establishment surface carry over onto the `[u8]`
carrier (the grant validator already exists).

## Execution order (the recipe)

1. **Keystone** — `semantics/omega-validation/src/expression_types.rs`: stop a
   string literal / `ExpressionNode::String` from satisfying `PrimitiveType::String`
   (lines ~98–99); it should satisfy only the `[u8] in Utf8` slice target (the
   line-70 path). This is what turns the tree red.
2. **The 15 sites** (below) — delete each `PrimitiveType::String` branch and lean on
   the already-present slice-descriptor / `&[u8] in Utf8` handling. The backend
   layout sites (`sizing.rs`, `runtime-storage/layout.rs`, `storage_places.rs`) and
   the interpreter ZII (`evaluator.rs` `Value::str(String::new())`) need the text
   descriptor to come from the slice-in-Utf8 path instead.
3. **Corpus** — migrate the `.omg` files (200 with literals; ~137 typed) off
   `string`/`String` to the carriers + rewritten operators. `.omg` edits don't
   rebuild Rust, so re-running `cargo test -p omega-compiler --test canary_suite`
   (~40 s) verifies each batch fast.
4. **`str.omg`** — re-express the owned surface as `Vec<u8> in Utf8` boundary
   operators (it stays gated, same as today); keep the borrowed surface as
   `&[u8] in Utf8` slice ops.
5. **Retire the type** — remove the `PrimitiveType::String` variant
   (`foundation/omega-core/.../types`), the two builtin registrations in
   `foundation/omega-core/src/symbols/builtin.rs` (`"String"` line ~75, `"string"`
   line ~96), and `PrimitiveType::from_name`'s String arm.
6. **Verify green** — full `cargo test --workspace` (build + 321 canaries +
   differential oracle) and the dungeon byte-identical to the interpreter.

## Inventory — the 15 live `PrimitiveType::String` sites

```
backend/omega-instruction-selection/src/selection/runtime_dispatch/guards.rs            (text-descriptor guard recognition)
backend/omega-instruction-selection/.../writes/mutation.rs                              (byte-width => None)
backend/omega-instruction-selection/.../writes/mutation/binary_table_writes.rs          (byte-width => None)
backend/omega-instruction-selection/.../writes/mutation/value_operands.rs               (fat-slice value-operand)
backend/omega-instruction-selection/src/selection/storage_places.rs                     (fat-slice place + TypeLayout text descriptor)
backend/omega-layout/src/sizing.rs                                                       (fat_descriptor_layout)
backend/omega-runtime-storage/src/layout.rs                                              (pointer_size*2 layout)
orchestration/omega-interpreter/src/evaluator.rs                                         (ZII Value::str, cast pass-through, classify guards)
pipeline/omega-symbol-resolved-trees-to-typed-trees/src/equatable.rs                     (FieldEquality::Text)
representations/omega-typed-trees/src/wire.rs                                            (WireScalar::Text)
semantics/omega-validation/src/expression_types.rs                                       (THE KEYSTONE — literal typing)
semantics/omega-validation/src/data.rs                                                   (data-field validation)
semantics/omega-validation/src/properties.rs                                             (property surface)
semantics/omega-validation/src/arithmetic_domains.rs                                     (exclude String from arith)
semantics/omega-validation/src/wire.rs                                                   (wire validation)
```

## Notes

- `~3–6` owned-growable usages are confined to library surface (`str.omg`,
  `vec.omg`, `fixed_vec.omg`) — none in samples; they ride the gated `Vec<u8>` path.
- This recipe is the main-compiler counterpart to the memory note
  `string-encoding-domain-model`; keep them in sync.

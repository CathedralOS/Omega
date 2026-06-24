# `String` retirement — execution recipe (#66 Phase B2–B4)

> **Status: NOT mechanical — bigger than first scoped (corrected 2026-06-22).**
> Examining the real corpus overturned an earlier optimistic read: **owned `String`
> is pervasively NATIVE**, not a gated afterthought. ~120 passing canaries — the
> **entire dungeon** and every runtime text canary (`runtime_string_concat_exit`,
> `runtime_text_builder`, `runtime_string_field_concat_exit`, …) — compile, run, and
> are oracle-matched *today* on owned `String` (fields, copy, concat-into-buffer,
> text builders). Retiring the type means migrating **all of that** onto *bounded*
> carriers (`FixedVec<u8>`/`[u8;N] in Utf8`) without regressing the dungeon, and the
> growable bits (`with_capacity`/`push_str`) genuinely need the allocator. This is a
> real, risky migration arc — **partly allocator-gated** — not a quick rip. `cb165c24`
> is the last green commit.

## Why it's atomic

The keystone (string literals satisfying `PrimitiveType::String`) cascades to every
`: string` / `String` declaration the moment it changes, and **all 15 compiler
sites are live** — `String` is *implemented as* the fat-slice/text descriptor
(`size = pointer*2`, `fat_descriptor_layout`, `text_descriptor`), and the
`&[u8] in Utf8` path was already added *beside* it (e.g. `guards.rs`:
"recognize such a slice-descriptor place **not** `PrimitiveType::String` too").
So there is no dead branch to delete incrementally; the variant and its uses come
out together.

## Allocator: a PARTIAL blocker (corrected — the earlier "not a blocker" was wrong)

The split that actually matters:

- **borrowed `&string`** → `&[u8] in Utf8` — ungated, the easy part.
- **owned-FIXED `String`** (struct/machine fields, field-copy, concat-into-buffer,
  text builders) — **lowers and RUNS today** via fixed/inline buffer storage, no
  allocator. This is the pervasive part: **~120 passing canaries + the whole
  dungeon**. Retiring it means migrating every one onto a bounded carrier
  (`FixedVec<u8>` / `[u8;N] in Utf8`, which ship today) **without regressing the
  runtime behavior or the differential oracle**. Ungated, but the hard, careful
  core of the arc — not mechanical.
- **owned-GROWABLE `String`** (`with_capacity`/`push_str`/`from_utf8`, the
  `boundary operator` surface in `str.omg`) → `Vec<u8> in Utf8` — **genuinely
  allocator-gated** (stage-1, not built). Few sites (the str/vec library surface),
  but this part cannot complete until the allocator does.

So the original instinct that the allocator was tangled up with full `String`
cleanup is correct: the growable surface waits on it, and the fixed surface is a
large careful migration of the live corpus + dungeon.

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

- Distinguish **growable** from **owned-fixed**: growable ops
  (`with_capacity`/`push_str`) are confined to library surface (`str.omg`,
  `vec.omg`, `fixed_vec.omg`) and ride the gated `Vec<u8>` path — but **owned-fixed
  `String` fields are everywhere** (~120 canaries + the dungeon) and are the real
  migration work (→ `FixedVec<u8>`/`[u8;N] in Utf8`).
- This recipe is the main-compiler counterpart to the memory note
  `string-encoding-domain-model`; keep them in sync.

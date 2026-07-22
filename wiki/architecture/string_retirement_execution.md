# `String` retirement — execution recipe (#66 Phase B2–B4)

> **Status corrected 2026-07-20: the generic domain-fact forwarding blocker is
> complete.** The three formerly missing pieces landed independently:
>
> 1. call arguments and other name expressions canonicalize to symbol-rooted
>    places, so a fact established on a parameter/local matches its forwarded use;
> 2. guarded-transition fallthrough preserves the pre-transition semantic
>    context; and
> 3. case construction plus matched-arm destructuring carries declared payload
>    domains into the payload binding.
>
> `runtime_param_domain_forward_exit` and
> `runtime_case_payload_domain_forward_exit` now compile and exit 70 in both the
> interpreter and native backend. `domains/domain_param_forwarded` exercises the
> same rule over `Blob::Scanned`, proving that forwarding is carrier/domain
> generic rather than a `Utf8` compiler special case. Declared-domain field,
> nested-field, indexed-field, literal, return-value, and subslice routes have
> their own canaries as well.
>
> The current critical path is therefore the migration itself: move remaining
> borrowed text to `&[u8] in Utf8`, move fixed owned text to bounded carriers,
> preserve the native/interpreter regression coverage, and then remove the
> builtin type and backend branches. Only the genuinely growable surface remains
> allocator-gated.
>
> Fixed-carrier equality is no longer a migration blocker. The shared
> representation-aware equality operands now read `{len, bytes}` carriers and
> `{ptr, len}` views correctly in literal, carrier-to-carrier, guard, local,
> forwarded-local, field-store, nested-boolean, and inequality positions on
> x86-64 and AArch64. The migrated regression canaries preserve interpreter
> oracles alongside native execution.
>
> ZII and copy routes are migrating on the same representation. A never-written
> `[u8; N] in Utf8` is empty text (`len == 0`), not an always-full N-byte zero
> array; native and interpreter execution now agree on that default. Nested
> case-payload equality, mutable carrier aliases, and copying a local record's
> carrier field through a mutable output parameter likewise no longer depend on
> builtin `String`.
>
> Mutable parameters are no longer a separate backend shape. Direct carriers,
> mutable carrier pointees, and nested carrier fields lower through the same
> Place-shaped append operations on x86-64 and AArch64. The in-place first segment
> is recognized as an alias and retained; it is never zeroed before appending.
> Immutable data parameters seed declared-domain facts for their nested fields,
> while writes through mutable parameters must prove or establish the destination
> field's domain at that write. The negative
> `state_parameter_field_domain_write_unestablished` canary pins that distinction.
>
> Frame argument materialization is now type-driven rather than size-driven.
> A by-value `[u8; N] in D` that happens to occupy 16 bytes remains the inline
> `{len, bytes}` carrier; only an actual slice/string descriptor receives the
> `{ptr, len}` rewrite. This closes the smallest-capacity case where layout size
> alone previously confused two unrelated representations.
>
> Bounded carriers now also cross ordinary machine-result and borrowed-view
> seams. A literal may directly satisfy a `[u8; N] in D` argument or terminal
> result only when its byte length is at most `N`; over-capacity returns fail at
> the construction site. A value call returning `[u8; N] in D` contributes `N`
> to the destination's static length proof, and projecting that owned carrier
> to `&[u8]` synthesizes `{ptr = &inline_bytes, len = runtime_len}`. It never
> reinterprets the carrier's leading length word as a pointer or exposes the
> full capacity as the live view length. The same projection follows a mutable
> reference to the carrier before reading its length or taking the inline-byte
> address; the reference slot itself is never reinterpreted as carrier storage.
> Guarded machine returns likewise construct string literals as owned
> `{len, inline_bytes}` values in bounded call-result slots before any caller
> copy. Interpreter/native differentials and AArch64 compilation pin both paths,
> including the migrated clear/carve/render and full-level-wrapper dungeons.
>
> Mutable boundary establishment is now carrier-generic. Named boundary/operator
> statement calls map declared parameters to exact caller places, invalidate
> facts for every mutable operand, and only then instantiate domain-membership
> `ensures` facts onto those places. `utf8_boundary_established` and
> `no_nul_boundary_established` now exercise `[u8]` carriers directly; the
> negative `boundary_operator_mutation_invalidates_domain` canary proves that a
> mutating boundary with no matching guarantee cannot preserve a stale fact.
> General boolean postcondition substitution remains ordinary frame work, not a
> text-retirement blocker.
>
> Lookup records are now on the carrier path as well. The ordinary and
> large-record lookup canaries (including the large room payload) store labels,
> descriptions, and output lines as bounded `Utf8` carriers and run through both
> engines. A machine that fills a record through `&mut` explicitly guarantees
> the returned field membership (`ensures out_room.label in Utf8`); callers do
> not retain a pre-call field fact across mutation. Capacity-specialized domain
> declarations share an unqualified lookup identity only when their normalized
> fact sets agree. Reusing the normalized name with different facts is a compile
> error rather than an order-dependent choice of declaration.
>
> The standard Console surface no longer owns text. `write` and
> `write_line` accept `&[u8]` and their checked adapters walk that view directly;
> the former `String -> &string -> bytes` adapter chain has been deleted.
> Bounded-carrier fields, literals, and legacy String callers all cross this seam
> in both engines, while a ZII bounded carrier arrives as an empty view.
> `read_line` accepts `&mut [u8]`; when the actual destination is an owned
> `[u8; N] in D` carrier, planning derives the writable inline capacity from
> that concrete place and writes its runtime length. It never applies the
> legacy 256-byte String scratch limit to a shorter carrier. Native/interpreter
> input differentials and AArch64 compilation pin the standard surface.

> **Migration-cost lesson:** this is not a mechanical keyword deletion. The
> historical corpus exercised owned `String` natively across fields, copies,
> concat-into-buffer, builders, and the dungeon. Those behaviors must move onto
> bounded carriers without losing their native/interpreter regression coverage.
> Migration is underway, but the growable `with_capacity`/`push_str` surface still
> genuinely needs the allocator. The arc is therefore partly allocator-gated,
> while fixed-capacity corpus migration and running-length proofs remain actionable.

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
  text builders) — lowers and runs through fixed/inline storage without an
  allocator. It was historically pervasive across the canary corpus and
  dungeon. Retiring it means migrating every remaining site onto a bounded carrier
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

1. **Corpus first, while compatibility remains** — migrate `.omg` declarations
   off `string`/`String` to borrowed or bounded carriers and preserve what each
   canary actually proves. This keeps batches reviewable and exposes missing
   carrier lowering before the builtin disappears. Re-run the focused native
   test and interpreter oracle for each batch.
   Two pass-canary sources still declare builtin `String`/`string` as of
   2026-07-21; derive the current count from the corpus rather than treating this
   snapshot as a completion condition.
2. **Keystone** — once source users are gone,
   `semantics/omega-validation/src/expression_types.rs` stops a string literal /
   `ExpressionNode::String` from satisfying `PrimitiveType::String`; it should
   satisfy only the carrier/domain targets.
3. **Compiler branches** — delete every remaining `PrimitiveType::String` branch
   and lean on the already-present slice descriptor or bounded-carrier handling.
   Layout, storage-place, calling-policy, wire, validation, and interpreter
   branches all remain live; derive the inventory from the tree rather than a
   copied count:

   ```powershell
   rg -n "PrimitiveType::String" compiler/omega-rs -g "*.rs"
   ```

4. **`str.omg`** — re-express the owned surface as `Vec<u8> in Utf8` boundary
   operators (it stays gated, same as today); keep the borrowed surface as
   `&[u8] in Utf8` slice ops.
5. **Retire the type** — remove the `PrimitiveType::String` variant
   (`foundation/omega-core/.../types`), the two builtin registrations in
   `foundation/omega-core/src/symbols/builtin.rs` (`"String"` line ~75, `"string"`
   line ~96), and `PrimitiveType::from_name`'s String arm.
6. **Verify green** — full `cargo test --workspace` (build + 321 canaries +
   differential oracle) and the dungeon byte-identical to the interpreter.

## Live compiler inventory

Do not maintain a hand-counted site list here: the compatibility implementation
has grown since the original 15-site survey. The search above is authoritative.
Classify every hit before deletion as one of:

- source typing/validation and builtin symbol registration;
- type properties, equality, recasts, arithmetic-domain exclusion, and wire
  classification;
- layout, runtime storage, calling-policy shape, and storage-place resolution;
- instruction selection for descriptors, bounded carriers, guards, writes, and
  value operands; or
- interpreter defaulting, casts, comparisons, and wire behavior.

Deletion is complete only when the search is empty and the carrier corpus plus
the negative tests cover the behavior previously pinned by each category.

## Notes

- Distinguish **growable** from **owned-fixed**: growable ops
  (`with_capacity`/`push_str`) are confined to library surface (`str.omg`,
  `vec.omg`, `fixed_vec.omg`) and ride the gated `Vec<u8>` path — but **owned-fixed
  `String` fields are everywhere** (~120 canaries + the dungeon) and are the real
  migration work (→ `FixedVec<u8>`/`[u8;N] in Utf8`).
- This recipe is the main-compiler counterpart to the memory note
  `string-encoding-domain-model`; keep them in sync.

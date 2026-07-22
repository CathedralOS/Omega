# `String` retirement — completed execution record (#66 Phase B2–B4)

> **Completed 2026-07-21.** Builtin `string`/`String`,
> `PrimitiveType::String`, `omega/language/core/str.omg`, and every
> compatibility branch are gone. The source corpus, samples, injected build
> vocabulary, wire path, interpreter, and both native backends use borrowed
> byte views or bounded carriers. The three formerly missing domain-forwarding
> pieces landed independently:
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
> Growable text remains ordinary allocator work (`Vec<u8> in Utf8`); it no
> longer keeps a compatibility primitive alive.
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

## Allocator: not a retirement blocker

The split that actually matters:

- **borrowed `&string`** → `&[u8] in Utf8` — ungated, the easy part.
- **owned-FIXED `String`** (struct/machine fields, field-copy, concat-into-buffer,
  text builders) — lowers and runs through fixed/inline storage without an
  allocator. It was historically pervasive across the canary corpus and
  dungeon. Retiring it means migrating every remaining site onto a bounded carrier
  (`FixedVec<u8>` / `[u8;N] in Utf8`, which ship today) **without regressing the
  runtime behavior or the differential oracle**. Ungated, but the hard, careful
  core of the arc — not mechanical.
- **owned-GROWABLE text** becomes `Vec<u8> in Utf8` and is genuinely
  allocator-gated. It is a future collection/library surface, not a reason to
  retain `String`, `str.omg`, or compiler compatibility branches.

The fixed surface was the retirement dependency and is complete. Allocation now
gates only the future growable feature itself.

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

## Completed execution order

1. **Corpus first, while compatibility remains** — migrate `.omg` declarations
   off `string`/`String` to borrowed or bounded carriers and preserve what each
   canary actually proves. This keeps batches reviewable and exposes missing
   carrier lowering before the builtin disappears. Re-run the focused native
   test and interpreter oracle for each batch.
   The pass-canary corpus has no builtin `String`/`string` declarations as of
   2026-07-21. The final two sources were in-place append regressions; a
   conservative straight-line reaching-length proof now admits each bounded
   chain that fits and still rejects a provable overflow. Samples, lattice
   inputs, legacy host contracts, deliberate fail fixtures, and the core
   compatibility surface remain separate migration owners; derive their live
   inventory from the tree rather than treating this snapshot as a completion
   condition. The standalone `text/string_catalog` sample and its lattice
   mirror now use bounded UTF-8 fields, including an explicit capacity for the
   concatenated label.
   The generic sample pause surface is also migrated: 120 scratch fields use
   `[u8; 256]`, matching the old native read ceiling rather than shortening the
   consumed line. The sample compile and documented-exit runtime sweeps cover
   the batch. The final eleven-file dungeon workload and its lattice mirror now
   use bounded UTF-8 carriers as well. Its reusable output scratch stays a
   domain-qualified carrier and each writer explicitly re-establishes `Utf8`;
   persistent room, player, enemy, input, and output fields retain
   declared domain facts. No sample or lattice-corpus source declares builtin
   `String` or `string` now. Calling-policy rejection data uses a bounded UTF-8
   carrier. The unimported `omega/host` scaffold was retired wholesale because
   it encoded superseded `capability`/`entry` architecture; the live standard
   boundary and target-provider homes are recorded in `omega/host/README.md`.
   The two standalone nested-command run fixtures now use bounded UTF-8 input
   and retain their native `look` result. The negative corpus is migrated too:
   tests that still express real borrow, domain, generic-bound, parser, provider,
   or wire constraints now use carrier-native types, while tests that existed
   only to reject operations on the retired owned primitive were deleted. Stale
   fail directories not enumerated by the canary suite were removed rather than
   preserved as misleading source examples. No fail-canary source declares
   builtin `String` or `string`. The core surface and compiler-injected build
   prelude were the final source owners and are carrier-native now.
2. **Keystone** — string literals stopped satisfying a magic primitive and now
   satisfy only literal, carrier, and domain targets.
3. **Compiler branches** — every `PrimitiveType::String` branch was deleted in
   favor of slice-descriptor or bounded-carrier handling. The authoritative
   retirement check is:

   ```powershell
   rg -n "PrimitiveType::String" compiler/omega-rs -g "*.rs"
   ```

4. **`str.omg`** — deleted. Borrowed operations are ordinary slice operations;
   future growable text is `Vec<u8> in Utf8` and will arrive with allocation.
5. **Retire the type** — removed the variant, both builtin registrations, and
   name conversion arms. A user may now declare ordinary data named `String`;
   a negative canary proves that spelling receives no hidden properties.
6. **Verify green** — the complete pass/fail canary sweeps, sample compile sweep,
   and documented-exit runtime sweep are green on Windows. Workspace-wide test
   coverage remains the final release gate for the milestone.

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

- Distinguish **growable** from **owned-fixed**: growable operations ride the
  future allocated `Vec<u8>` path; bounded ownership already uses
  `FixedVec<u8>`/`[u8; N] in Utf8` throughout the corpus and dungeon.
- This recipe is the main-compiler counterpart to the memory note
  `string-encoding-domain-model`; keep them in sync.

# Design Brief — Programmable Layouts (plans, projections, and the retirement of `wire data`)

> **For:** Omega maintainer · **Status:** SETTLED (chat session 2026-07-02, Zach) —
> all syntax provisional; the *mechanism decisions* are the settled part. ·
> **Driver:** UEFI/boot needs C-ABI byte layouts without importing C into the
> language (the Rust `repr(C)` dependency is rejected outright), and the
> `wire data` construct was carrying serialization concerns that belong at use
> sites. · **Amends:** [`extern_boundary_and_format_domains.md`](extern_boundary_and_format_domains.md)
> §5/§6/§9/§10 (catalog → policies; wire data retired) and
> [`freestanding_boot_and_hardware_facts.md`](freestanding_boot_and_hardware_facts.md)
> sample 2. · **Depends on:** the ch13 build-time-evaluation sketch (const evaluation +
> member reflection), [`build_time_evaluation.md`](build_time_evaluation.md), the totality budget story,
> and [`index_count_and_address_model.md`](index_count_and_address_model.md)
> (`count`/`addr`; no `usize`).

---

## 1. Bottom line up front

**Layout is a library, not a language axiom.** A layout policy is ordinary data
satisfying a `Layout` trait whose `plan()` machine runs at build time (zero
effects, structurally bounded) and returns a `Plan` — a per-field placement
description in a **closed vocabulary** the compiler knows how to walk. The
compiler **validates** the plan (no overlap, no straddle, ranges fit slots),
then **derives** everything from it: codec, field projections, mint-time
indexes, value types. The C ABI becomes ~15 audited lines of Omega
parameterized by target facts — re-implemented, never imported. `repr(C)` has
no analog; neither does `unsafe` casting.

Consequences that fell out and are settled here:

1. **One declaration form: `data`.** The `wire data` construct is **retired**.
   Field numbers become *optional identity syntax* on plain `data`; `retired N;`
   tombstones a number. Policies consume the schema facts their grammar needs
   (declaration order for C, numbers for tagged wire) and ignore the rest.
2. **Formats live at use sites** (carriers / type position), with grammar
   selection defaulted from schema facts (`OmegaLayout`).
3. **Bit-addressable fields with no bit-width types** — `Bits` placements +
   range facts on plain integers; shifts live in the deriver.
4. **Durability is a plan grade consumed at build time by typed APIs** — never a
   domain on bytes (two attempts at byte-level `Durable`/`SelfDescribing`
   domains were made and rejected; recorded in §9 as anti-patterns).

## 2. The trait, the policy, the plan machine

`Layout` is a pure interface; a policy is a data type satisfying it; the
machine does the work. No trait carries a body.

```omega
trait Layout {
    machine plan(schema: Schema) -> Plan;
}

data CLayout;                       // a policy: zero-field tag type
CLayout satisfies Layout {
    machine plan(schema: Schema) -> Plan {
        transition schema.fields.len > 0 {
            true  -> place(0, 0, 1, FieldOffsets::empty())
            false -> done(0, 1, FieldOffsets::empty())
        }
        state place(index: count, offset: count, align: count, offsets: FieldOffsets) {
            let field = schema.fields[index];
            let at: count = round_up(offset, field.align);       // the C rule
            let widest: count = max(align, field.align);
            transition index + 1 < schema.fields.len {
                true  -> place(index + 1, at + field.size, widest, offsets.push(field.name, at))
                false -> done(at + field.size, widest, offsets.push(field.name, at))
            }
        }
        state done(end: count, align: count, offsets: FieldOffsets) -> Plan {
            Plan { offsets: offsets, size: Fixed(round_up(end, align)), align: align }
        }
    }
}
```

- **Iteration is states** (self-re-entering with accumulators) — the decreasing
  measure (`fields.len - index`) is the build-time termination proof, falling out
  of the shape rather than bolted on. No `for`, no `var`.
- **Sizes and offsets are `count`s** (no `usize`; field metadata arrives
  target-resolved — pointer width, natural alignments — so no ABI threading).
- **Build-time safety is the effect system**: `plan()` declares no effects, so
  it *has* none — no IO at build time, structurally. Evaluated by the reference
  interpreter (the ch13 build-time-evaluation story). The only misbehaviors possible are a
  garbage plan (caught by validation, §3) and non-termination (totality budget).
- **Reflection ask**: `Schema` as first-class build-time reflection data (fields with
  name/size/align/type-kind/number). A modest generalization of the ch13
  `Self::fields` sketch — the reflection surface was already footnoted open.
- `Packed` is the same machine minus one `round_up`. The C ABI's pathological
  corners (bitfields-in-C's-sense, unions) are simply not implemented — the
  clean 90% UEFI/Win32 actually uses lives in the library; horror is refused
  rather than swallowed.

## 3. The Plan: a closed placement vocabulary, validated, then trusted

```omega
data FieldPlan {
    case At(offset: count);                                     // byte-aligned fixed
    case Bits(container: count, container_width: count,
              lsb: count, width: count);                        // bit slot in a container word
    case Varint(tag: count);                                    // tagged scan
    case LengthPrefixed(tag: count);                            // tagged scan, blob borrows
}

data Plan {
    fields: [FieldEntry];       // keyed by FIELD NAME, not position (reorder-proof;
                                //   a rename errors loudly at validation)
    retired: [count];           // tombstoned identity numbers (see §7)
    size: SizePlan;             // case Fixed(count) | case Dynamic
    align: count;
    // per-field access class (MMIO): ReadWrite (default) | ReadOnly | W1C | ReadPop | ...
}
```

**Programmable policy, checked output, derived everything.** Before any
codegen trusts a plan, the compiler validates: fields resolve to real schema
names, no overlap, in-bounds, no `Bits` straddling its container, field value
ranges fit slots (`2^width ≥ range`), tag collisions and retired-number reuse
rejected. A malicious or buggy policy can produce a *compile error*, never
memory unsafety. The vocabulary being **closed** is what keeps the conformance
theorem: the deriver owns what each shape means in bytes; policies only choose
shapes and positions. A grammar the vocabulary can't express (bit-packed
Huffman, custom compression) drops to tier 3 — an ordinary machine writing
bytes, ordinary proofs, no theorem — honest, not blessed.

**Computed vs stated plans are the same artifact.** `CLayout` computes;
hardware states literals (positions come from the datasheet, not an
algorithm):

```omega
data UartMmio;
UartMmio satisfies Layout {
    machine plan(schema: Schema) -> Plan {
        Plan { fields: [
            field("data",   At(0), ReadPop),      // access classes are PLAN DATA,
            field("status", At(5), ReadOnly),     //   not schema syntax
            field("ctrl",   At(3), ReadWrite),
            field("intr",   At(2), W1C),
        ], size: Fixed(8), align: 1 }
    }
}
```

Auditing a stated plan = reading literals against the SDM/datasheet — the same
trust shape as asm instruction contracts.

## 4. Derived consumers: one plan, three artifacts, agreement by construction

```text
policy plan(schema) ──► validated Plan ──┬──► encode / materialize   (walks every entry)
                                         ├──► field projections      (At/Bits entries)
                                         └──► mint-time offset index (Dynamic reads)
```

Because all three walk the *same* plan, `buf.gold == materialize(&buf).gold`
is a theorem, not a test. Decode stays the settled two-step: **validate**
(fallible mint, `&[u8]` → refined borrow) then **materialize** (total).

**Field projections** exist exactly where the plan makes them honest:

- `At`/`Bits` fields: real O(1) access on refined bytes and on values —
  `self.buf.physical_start` is a read at a validated offset, legal because the
  domain is established, dead again after a `&mut` boundary pass
  (forget-on-boundary-write). Writes additionally require **fixed width**
  (an `At`/`Bits` slot) — a varint write is a re-encode wearing assignment
  syntax and is not derived.
- `Dynamic` fields: no compile-time offset exists (it depends on the bytes) —
  but the **validating mint already scans every byte**, so it records the
  offsets it discovers (simdjson-tape / FlatBuffers-vtable trick). Read
  projection on dynamic formats is resolved at *mint time*, O(1) at access.
  Table-building is a mint variant (`validate` vs `validate_indexed`) chosen at
  the use site.
- The general rule: **every fact resolves at the earliest moment its inputs
  exist** — (schema × policy) → compile time; (× bytes) → mint time; access
  pays nothing.

## 5. Type-position policies: values laid out per plan

A policy with a **fully static plan** (every field `At`/`Bits`, size `Fixed`)
is legal in type position — a real value type whose in-memory layout *is* the
plan, with derived native field access:

```omega
save: Save;                            // default: sovereign layout, no policy anywhere
gdt:  CLayout<GdtDescriptor>;          // policy-laid value — ZII gives a valid zeroed entry
msg:  Protobuf<Save>;                  // ERROR: Dynamic plan cannot be a value type —
                                       //        values need offsets, bytes need mints
```

- Construction: ZII, literal, or field-copy from the native value; `.into()`
  back. No mint — it's our value, not untrusted bytes.
- **The no-op boundary theorem**: a `&CLayout<T>` *is* `&[u8] in CLayout<T>` —
  its bytes are conformant by construction, so handing it across an FFI/firmware
  edge is a borrow, not an encode. (Generalizes the extern brief's structural-
  domain zero-copy case: a policy can *make* representations coincide. With
  §5b's recast this stops being a separate theorem: it is the read-only corner
  of the recast judgment.)
- Serialization stays a carrier phenomenon: a value has exactly **one**
  in-memory form; multi-format is expressed by encoding into differently-
  refined carriers, never by multiplying value layouts.

## 5b. Recast: borrows under a second stated shape (settled 2026-07-02, chat)

**There is no reinterpret cast, no `bytes()` builtin, and no implicit borrow
coercion. A recast is a borrow spelled with `as`, and its legality is a
static judgment between two stated layouts.** `as` on a VALUE converts
(`x as f32` → `4000.0`); `as` on a BORROW re-views (`&x as &f32` → the bit
pattern) — a borrow is a place, and a place is never converted, only revealed
under another shape.

```omega
let int_bytes: i32 = 4000;
let as_float = &int_bytes as &f32;        // scalar pun, explicit, checked

data GdtRaw {                             // a second shape over Gdt's footprint
    head: [u8; 8];                        // relaxed: no facts here in Gdt either
    flags: u8 [0..=15];                   // KEPT verbatim: the fact-carrying region
    tail: [u8; 8];
}
let raw = &mut self.gdt as &mut GdtRaw;   // struct recast; nothing runs
let bytes = &mut self.gdt as &mut [u8; 24]; // the degenerate shape: [u8; N] IS a
                                            // (trivially) stated layout — legal iff
                                            // Gdt declares no facts anywhere
```

The judgment (all static, plan-level — same family as plan validation; a bad
relation is a compile error, never unsafety):

1. **Same footprint** — the target's plan tiles the source's plan exactly.
2. **Fact implication per overlapping region** — a `&` recast needs
   source-facts ⟹ target-facts; a `&mut` recast needs BOTH directions
   (anything writable through the target must leave the source valid at
   release). Fact-free regions are free; regions kept verbatim are free.
3. **Both types stated** — primitives and `[u8; N]` are trivially stated
   (target-public representations); records must be plan-laid. Sovereign
   layout stays sovereign: no recast can observe it.
4. **A recast may WEAKEN facts, never STRENGTHEN them.** `&mut ranged as
   &mut f32` rejects (a write could break the range); `&bytes as
   &([u8] in Utf8)` rejects absolutely — the extern brief's §7 trapdoor
   stays shut. Establishing a fact is a MINT's job: fallible,
   case-returning, forced-dispatch.
5. **Untyped sources annotate first** — recasting a local whose type came
   from literal inference is an error until the type is spelled.

Consequences: borrow exclusivity gives fact-suspension for free (while the
recast borrow lives, the source is inaccessible as its typed self); inline
asm consumes recast views and speaks contracts over the STATED shape (the
interrupt frame is the same story inbound); and the derived codec surface
shrinks to its honest core — **re-views are `as`, transforms are derived
machines**, and a grammar needs derivation exactly when it is not
fixed-layout (the tagged evolution grammar and nothing else).

## 6. Bits: bit-addressable fields, no bit-width types

Surface is bit-addressable; the machine is byte-addressable; **shifts live in
the deriver** (fused to `UBFX`/`BFI`/`BEXTR` where the ISA has them). Prior
art honestly: C bitfields exist but are unportable-by-spec; Ada rep clauses
and Zig packed structs are the real precedents.

- **No `u3` types** — same rule as the index model: *width is a lowering
  detail; the surface carries range facts on plain integers.* A 3-bit field is
  `u8 < 8` placed in `Bits(..., width: 3)`. Reads project a plain `u8` with
  the range fact **established free** (the mask is the proof). Arithmetic is
  ordinary; write-back re-discharges the range obligation — identical to any
  invariant-carrying field (relax-scopes applies). No casts; no promotion
  rules; overflow = failed obligation, never silent wrap.
- Stated plans put page-table entries at datasheet positions
  (`Bits(0, 64, 12, 40)` for the frame) and validation rejects straddles.

**Sub-word writes are read-modify-write — three hazards, three existing answers:**

| Hazard | Mechanism | New surface? |
|---|---|---|
| lost update, exclusive memory | `&mut` = exclusive; RMW is invisible detail | none |
| lost update, shared memory | atomics-only sharing → whole-word CAS; sub-word RMW *unwritable* | none |
| MMIO semantics (W1C, read-side-effects) | per-register **access classes in the plan**, consumed at *derive* time | plan data only |

MMIO enforcement is **absence, not rules**: `uart.intr.overrun = false` on a
W1C register *does not exist*; the derived surface is the shaped op
(`acknowledge(mask)` — stores the mask, never reads). `ReadPop` registers get
a consuming machine (`take()`), not a projection. And projections operate on
**snapshot values**, never the device: one volatile load → project freely →
one volatile store (`Mmio::load` / `Mmio::store` of whole containers). Access
classes are not effects (effects are per-machine behavior classes; W1C-ness is
per-register, datasheet-static) and not `requires` (nothing is dynamic).

## 7. `wire data` is retired

**One declaration form: `data`.** What `wire data` was actually providing —
durable field identity — becomes *optional identity syntax* on plain `data`;
everything else it implied (serialization gatekeeping, a second declaration
form, Omega-framing privilege) dissolves into policies.

```omega
data Save {
    1: seed: u64;          // optional identity numbers: any values, any order, sparse ok;
    retired 2;             //   unique + not-retired enforced. gold lived here once.
    3: name: [u8];
    4: mana: u32;          // added later: fresh number; 1/3 collide, 2 retired — all loud
}
data Scratch { pos: Vec2; zoom: f32; }     // unnumbered: no identity, and NEVER order-derived
```

- **Numbers are inert schema facts** — consumed only by identity-keyed
  grammars. In-memory layout (sovereign), proofs, ZII, `==`, CLayout (which
  consumes declaration *order* instead): all ignore them. Each policy reads
  exactly the facts its grammar needs.
- **`retired N;` not tombstone fields**: a dead field kept in the schema leaks
  into every consumer (CLayout would *place* it; Equatable/Hash/reflection all
  need skip-rules; its type reference keeps dead types alive). `retired` is
  absent from `schema.fields`, so every consumer ignores it by construction —
  the one fact that must survive a field's death is "this number is taken."
- **Evolution walkthrough** — kill: delete the field + `retired N;` (plan
  validation makes forgetting the schema edit loud; a **publish-time diff
  against the predecessor artifact** catches delete-without-retire, which no
  single compile can see). Add: new field + fresh number. Skew behavior both
  directions: unknown tag → skipped; absent tag → ZII. Adding is safe *exactly
  when zero-means-empty is right for the field* (`[zero_init]` policy);
  otherwise it's a version-block migration wearing add-clothing.
- Renames free (identity is the number). Re-meaning a field = kill+add or a
  ch21 version block + migration machine; the tag layer never pretends
  re-meaning bytes is safe. Version blocks and tag identity divide the labor:
  tags = compatible evolution both directions (including *forward* skew, which
  version chains cannot do — old code can't hold a decode for a shape that
  didn't exist); version blocks = breaking rewrites.

## 8. `OmegaLayout`: one exposed name, grammar as a defaulted parameter

```omega
a: [u8; 64] in OmegaLayout<Save>;            // grammar = Derived (default):
                                             //   numbered → tagged, unnumbered → packed
b: [u8; 40] in OmegaLayout<Save, Packed>;    // explicit: ignore numbers, most compact —
                                             //   same-version bytes (cache, shm ring)
c: [u8; 32] in OmegaLayout<Scratch>;         // unnumbered → packed
```

- **Detection is not a modality; it's the default value of an ordinary
  build-time parameter** (named, not bool). One name exposed; usually zero —
  Omega-native edges imply it.
- **One-way asymmetry**: you can always *drop* identity from the wire
  (`Packed` on a numbered schema); you can never *invent* it (no
  tagged-for-unnumbered option — order-derived tags are the corruption trap
  this design exists to kill). Identity flows from stated schema facts to
  bytes, never conjured at a carrier.
- Frames that can outlive a compile carry the **schema content-hash
  fingerprint**; the mint checks it first, so stale bytes fail as a
  deterministic `Invalid` (regenerate), never a misparse.
- The silent-flip hazard of detection is fenced structurally: non-durable
  bytes can only live on same-version edges (see §9), where encoder and
  decoder are one closure and recompile together.

## 9. Durability is a plan grade, not a byte domain

The deriver grades each resolved plan: identity-keyed placements → **durable**
(survives schema evolution); positional/offset placements → not. The grade is
consumed **at build time, by APIs whose contract is longevity**:

```omega
machine write(bytes: &[u8]) effects filesystem;   // raw edges: agnostic, correctly —
                                                  //   PNGs, ciphertext, foreign frames

data Store<T>;    // versioned persistence: build-time-requires plan(T) durable
                  //   Store<Save> OK; Store<Scratch> ERROR: no field identities
data Cache<T>;    // no check; read = validate → Valid | Invalid → regenerate
```

**Anti-patterns, recorded because both were tried in-session and rejected:**
byte-level `Durable` and byte-level `SelfDescribing` domains. Plan-level facts
do not attach to values — a raw write API doesn't care what bytes "are"; typed
inbound reads get **zero guarantees before the mint** (trust boundary: read,
attempt validate, `Valid | Invalid`); and bytes cannot attest history ("tags
never reassigned" is a publish-time property of the schema lineage, checkable
only where history exists). The only byte-level facts are **concrete layout
domains** (`in OmegaLayout<Save>`), established by encode outbound and by the
mint inbound. Raw positional persistence stays legal — own the consequences;
the fingerprint makes the failure clean.

Foreign edges are orthogonal to durability: `CLayout<T>` is *not* durable
(offsets shift under schema edits, no skip/absent machinery) and doesn't need
to be — a UEFI descriptor never crosses an evolution edge; its layout is
pinned by an external frozen spec, checked by the provides-mapping/boundary
contract instead.

## 10. What this asks of the language (delta over existing sketches)

1. **Build-time evaluation of effect-free machines** in constant position +
   totality budget — already the ch13/build-time-evaluation-brief direction; this is its
   first heavyweight client.
2. **`Schema` as first-class build-time reflection data** (name/size/align/kind/
   number per field) — generalizes the sketched `Self::fields`.
3. **Plan validation pass** + the **deriver** (codec, projections, mint-index,
   value types) — compiler-owned; the conformance theorem lives here.
4. **Optional field numbers + `retired N;`** on `data`; delete the `wire data`
   form (ch20 rewrite; extern brief §9 already demoted it to "schema identity",
   §10.5 asked whether the name survives — answer: no).
5. **Publish-time predecessor plan diff** (identity preservation) — package/
   artifact-level, not a compile check.
6. Access-class vocabulary in plans consumed by the MMIO deriver; SVD import
   as a future schema source is a plausible freebie.

Ripples: extern brief §5/§6 (catalog → authored policies, one level down:
blessed *placement primitives*, programmable compositions), §8 (structural-
domain zero-copy generalized by §5 here), boot brief sample 2 (loses the
`wire` keyword; `Uefi` becomes a stated/CAbi-derived policy; adds the two boot
deltas: runtime `DescriptorSize` stride — the spec *forbids* sizeof-striding,
which the obligation system can enforce — and BootServices function-pointer
tables re-opening the win64 call encoder at runtime pointers).

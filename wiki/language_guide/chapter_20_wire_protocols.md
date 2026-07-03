# Chapter 20: Wire Protocols

Omega treats protocol schemas as first-class source-visible contracts.

Wire protocols are about stable external representation: bytes on a socket,
messages on a bus, persisted packets, RPC payloads, logs, and cross-version
communication with peers outside the current process image. They are not the
same thing as runtime data layout.

The design splits into two independent concerns, each with exactly one home:

- **Field identity** — which field is which, durably, across schema history.
  This is a property of the schema itself, so it lives **in the declaration**:
  optional identity numbers on plain `data`.
- **Byte grammar** — how a value becomes bytes (varints, tags, offsets,
  framing). This is a property of each *edge*, so it lives **at the use site**:
  a layout policy named (or implied) at the carrier
  (see [Memory Layout And ABI](chapter_19_memory_layout_abi.md) and
  `design_briefs/programmable_layouts.md`).

There is no separate wire declaration form.[^wire-data-retired] One `data`
keyword declares every shape; schemas that cross evolution-durable edges add
identity numbers; policies consume the schema facts their grammar needs and
ignore the rest.

[^wire-data-retired]: `wire data` was retired as a construct on 2026-07-02
(`design_briefs/programmable_layouts.md` §7). What it actually provided —
durable field identity — became optional syntax on plain `data`; its
serialization role moved to layout policies; its schema-identity checking,
tag diagnostics, and codec generation carry over unchanged in substance. The
implemented encoder slice below still parses the legacy `wire data` form;
migrating the surface (declaration form, `reserved` → `retired`) is
mechanical and tracked in TASKS.

## Field Identity

Identity numbers are optional per-field syntax on `data`:

```omega
data CounterMessage {
    1: counter: i32;
    2: timestamp_millis: i64;
}

data Scratch { pos: Vec2; zoom: f32; }     // unnumbered: no identity contract
```

Working interpretation:

- Numbers may take any values, in any order, sparse or dense. The constraints
  are unique and not-retired.
- Field numbers are the wire identity; declaration order is not. Renaming a
  field does not change its identity.
- Numbers are **inert schema facts**: in-memory layout (compiler-sovereign),
  proofs, ZII, equality, and order-consuming policies (the C layout) all
  ignore them. Only identity-keyed grammars read them.
- An **unnumbered schema has no identity — never an order-derived one.** No
  grammar may invent tags from declaration order; that is the silent-
  corruption trap this design exists to exclude (delete a field, and an old
  payload decodes *validly* into the wrong fields with no error anywhere).
  Unnumbered schemas simply cannot cross evolution-durable edges; the error
  names the fix.

## Retiring A Field

```omega
data CounterMessage {
    1: counter: i32;
    retired 2;                 // a field lived here once; the number is burned
    3: timestamp_millis: i64;
}
```

- `retired N;` tombstones an identity number. Declaring a field with a retired
  number is a compile error.
- Retirement is a declaration, **not a tombstone field**: a dead field kept in
  the schema would leak into every consumer (the C layout would place it,
  derived equality/hash/reflection would each need skip-rules, and its type
  reference would keep dead types alive). `retired` is absent from the field
  list, so every consumer ignores it by construction.
- Deleting a field without retiring its number is invisible to a single
  compile (the compiler cannot see undeclared history). Two mechanisms close
  the gap: declared `version` eras make retiring a documented number
  enforceable in-compile, and the **publish-time predecessor diff** (package/
  artifact level) checks every identity in the previous artifact's plan is
  either present-with-the-same-field or retired.

## Grammars Are Layout Policies

Encoding is chosen at the EDGE, not on the declaration — and you hold VALUES,
not encoded bytes (settled 2026-07-02: a layout domain on owned storage would
be a trivially-claimed membership; domains on bytes are MINTED facts riding
borrowed views):

```omega
save: CounterMessage;                        // you own it: a VALUE, sovereign layout,
                                             //   no bytes, no domain, nothing named
// outbound: encode at the edge into plain scratch bytes
CounterMessage::encode(&save, &mut scratch, &mut written);
// inbound: the validate MINT makes the fact true and hands you a refined VIEW
//   case Valid(view: &[u8] in OmegaLayout<CounterMessage>)   — then materialize (total)
// grammar is a defaulted build-time parameter of the instance name:
//   OmegaLayout<CounterMessage>          — Derived (default): numbered → tagged
//   OmegaLayout<CounterMessage, Packed>  — explicit: ignore numbers, densest form,
//                                          same-version bytes only
```

- `OmegaLayout` is the one Omega-native policy family. The grammar is an
  ordinary defaulted build-time parameter (named, not bool); *detection is just
  the default value*, computed from a visible schema fact (the numbers are in
  the declaration). Omega-native edges imply the policy, so most code names
  nothing.
- The asymmetry is one-way by design: identity can always be **dropped** from
  the wire (`Packed` on a numbered schema — caches, same-version shared-memory
  rings) and can never be **invented** at a carrier.
- Foreign grammars (`Protobuf`, C-ABI layouts, …) are sibling policies over
  the same schemas; a provides-mapping to a foreign symbol implies its format
  (see the extern brief). All policies produce *plans* validated and derived
  by the compiler — encoding and decoding are generated from source-visible
  contracts, never hand-written byte code.
- Decode is the standard two-step: **validate** (fallible mint over `&[u8]`,
  producing a refined view) then **materialize** (total). Inbound bytes carry
  zero guarantees before the mint — a trust boundary, not a type assertion.

## Durability Is A Plan Grade

The deriver grades each resolved plan: identity-keyed placements survive
schema evolution (**durable**); positional or offset-based placements do not.
The grade is consumed **at compile time by APIs whose contract is
longevity** — it is never a fact about bytes:

- A versioned store (`Store<T>`-shaped API) build-time-requires a durable plan;
  handing it an unnumbered or `Packed` schema is a compile error naming the
  fix.
- Raw byte edges (`write(bytes: &[u8])`) are format-agnostic, correctly —
  ciphertext, images, and foreign frames are all legitimate bytes.
- Cache-shaped APIs accept non-durable frames; persisted non-durable frames
  carry the schema's content-hash fingerprint so stale bytes fail as a
  deterministic decode `Invalid` (regenerate), never a misparse.
- The C layout is *not* durable and does not need to be: a foreign-ABI struct
  never crosses a schema-evolution edge — its layout is pinned by an external
  frozen spec, checked by the boundary contract instead.

## Compatibility Rules

Schemas with identity numbers evolve under explicit rules. The safe path is
easy:

- Adding a field with a fresh number is compatible: old payloads decode with
  the new field absent → ZII. This is safe exactly when zero-means-empty is
  the right reading for the field; when a zero default would be wrong, the
  change is a version-era migration wearing add-clothing, and the honest tool
  is a `version` block.
- Removing a field retires its number (above).
- Renaming a field is compatible; identity is the number.
- Changing a field's type, presence semantics, or defaults is a protocol
  change requiring an explicit rule — within an era it is an error; across
  declared eras it is legitimate evolution reported as "requires migration."
- Unknown fields may be rejected, ignored, or preserved by schema policy. The
  same policy covers unknown case tags on case-bearing fields (a newer era's
  case arriving at an older reader): reject, preserve raw, or decode as the
  zero case. In-language match exhaustiveness is never weakened for this —
  cross-binary openness is a wire decode policy, not a type-system property.
- Tag identity and version eras divide the labor: **numbers handle compatible
  evolution in both directions** — including *forward* skew (old code reading
  a newer peer's bytes by skipping unknown tags), which version chains cannot
  do — **`version` blocks handle breaking rewrites.**
- Compatibility checks run along the version chain: each declared era is
  checked against its successor, matching how migrations compose in
  [Versioned Data](chapter_21_versioned_data.md).

Declared versions change the reservation story. Generated tagged encodings
always carry an ERA DISCRIMINATOR (one varint per top-level message or record
— never per struct, never in native layout); a schema with no version blocks
encodes era `0`, and introducing versioning later snapshots the old body as
that era, so pre-versioning data stays decodable. Because the decoder always
knows a message's era:

- `retired` protects against ACCIDENTAL reuse within an era; deliberate
  cross-era recycling of a retired number is legal — the era tables
  disambiguate what proto must treat as radioactive forever.
- A field number changing type ACROSS eras is legitimate evolution (decode via
  the old era's table, migrate up the chain), not a compile error. Hard errors
  are for within-era violations and declared-history contradictions.

## Wire Versions

Version blocks exist for genuinely distinct eras, not for every schema change:

```omega
data CounterMessage {
    version v1 {
        1: counter: i32;
    }

    retired 1;
    2: counter: i32;
    3: timestamp_millis: i64;
}
```

Version blocks are useful for breaking protocol eras, compatibility with an
already-shipped external format, protocol envelopes carrying an explicit
version, and decode paths where old payloads need different validation. They
are not a replacement for stable field numbering — if every field change
creates a new era, the protocol becomes harder to evolve and interoperate
with.

## Message Shapes And Runtime Shapes

With one declaration form, a schema may be serialized directly, or a protocol
may keep a distinct message shape optimized for compatibility next to a
runtime shape optimized for execution:

```omega
data Counter {                    // runtime shape
    counter: AtomicI32;
    timestamp: DateTime;
}

data CounterMessage {             // message shape
    1: counter: i32;
    2: timestamp_millis: i64;
}
```

Conversion between them is ordinary machines with ordinary obligations — no
special `encode`/`decode` keywords for transform code, no blessed conversion
trait required. Wire decoding (bytes → validated message value, generated
from the plan) and runtime conversion (message value → runtime value, written
by hand) stay distinct operations: the former carries protocol obligations
(validation, compatibility, unknown fields, canonical encoding), the latter
carries the usual effect, ownership, and invariant obligations. Runtime
hot-swap migration is a third thing with its own obligations
([Versioned Data](chapter_21_versioned_data.md)).

## The Implemented Encoding: `compact_binary` v0

STATUS: the first implemented grammar — the **tagged grammar of
`OmegaLayout`**. The domain-instance spelling parses and validates
(`OmegaLayout<Schema>`: the schema must be identity-numbered — the packed
grammar of an unnumbered schema is not implemented; an explicit grammar
argument rejects, `Derived` being the default and only grammar), and it obeys
**mints-only** (§ "domain entry"): the domain rides BORROWED VIEWS
(`&[u8] in OmegaLayout<Schema>`, the validate-mint's result payload — the
mint itself is up-ladder), never owned storage. Declaring it on a stored
`[u8; N]` is a compile error — a zeroed buffer holds no valid encoding, so a
declared refinement would be a trivially-claimed membership. You hold the
VALUE (`save: Save`) and encode at the edge; buffers are plain bytes. A
refined view is a plain byte view to layout and codegen — never the
`{len, bytes}` text carrier. The synthesized `Schema::encode(&value, &mut out, &mut
written)` encoder covers primitive integer fields (i32, i64, u32, u64, bool):
the message's ERA DISCRIMINATOR varint comes first, then each current-era
field in field-number order as a field-number varint followed by a value
varint, where varints are unsigned LEB128, signed values zigzag first
(`(n << 1) ^ (n >> 63)`, so small negatives stay short), and bool encodes as
one byte 0/1. The out buffer must be a `&mut [u8; N]` large enough for the
worst-case encoding (checked at compile time, so the encoder needs no runtime
bounds checks), and `written` receives the encoded byte count.

A `String` field rides as its tag varint, then a LENGTH varint (byte count),
then the raw UTF-8 bytes — no NUL terminator, no padding. String fields are
ENCODE-ONLY today, and the encoder takes at most one per message, carrying
the schema's highest field number so it encodes LAST. Both restrictions fall
out of the same fact: a String's byte count is runtime-sized (the value is a
`{ptr, len}` text descriptor), so it cannot participate in the compile-time
worst-case capacity check. The worst-case budget covers everything up to and
including the length varint (ten bytes max); the trailing byte-copy is the
one append that bounds every store against the buffer's compile-time length
at runtime, DROPPING content past capacity rather than writing out of bounds
(callers size buffers for their longest expected text — a runtime overflow
signal for encode is future work). Decode REJECTS String fields for now.
The honest storage options were: (a) zero-copy — write a descriptor pointing
INTO the decode buffer, which makes the decoded message silently alias the
buffer; today's borrow facts track view loans created by explicit slice/text
borrow expressions only, so the checker CANNOT see a call output retaining a
borrow of another argument, and mutating or reusing the buffer would
invalidate the decoded string with no diagnostic; or (b) reject decode until
that aliasing relationship is checkable (or an allocator/copy target exists).
We took (b): encode-only is a smaller honest slice; zero-copy decode awaits
borrow facts that can model it (tracked in TASKS).

The matching decoder is
`Schema::decode(&mut value, &buffer, &mut read, &mut ok)`: it reads the
era varint, then per field the expected field-number varint and a value
varint, un-zigzagging signed fields, and writes each value into the matching
field of `value`. `read` receives the byte count consumed and `ok` the
success flag. The decoder accepts the schema's CURRENT era only — a payload
carrying any other era discriminator fails on its first byte; decoding
historical eras is deferred until the `Versioned<T>` container (chapter 21
stage 3) is signed off, since ordinary values cannot carry an era tag.
Failure semantics: `ok` is sticky — the first violation (wrong era, a tag
that is not the next expected field number, truncated input, or an overlong
varint past ten groups) makes the decode report failure, and nothing can set
the flag back. On failure the decoder guarantees only the flag: `read` and
the message's fields may reflect a partial or garbage decode (no rollback),
but every byte read is bounds-checked against the buffer's compile-time
length, so a failed decode never reads out of bounds.

A NESTED MESSAGE field — a field whose type is another schema, like
`1: header: RoomHeader;` — rides as its tag varint, then a byte-LENGTH
varint, then the sub-message's fields (tag + value pairs) WITHOUT an era
discriminator. Decision 10's frozen text settles the framing: one era varint
per top-level message, NEVER per struct. The era rides only the top-level
envelope; a nested schema's version chain is checked at its own top-level
uses, and its declaration is validated like any other schema's. Today's
honest slice is ONE nesting level with a scalar-only child body (i32, i64,
u32, u64, bool): a String child is runtime-sized and a doubly-nested child
would need a second staging region, so both reject with clear diagnostics,
and a schema that reaches itself through nested fields (no finite worst case)
is a hard error at the declaration.

The length prefix is the interesting part: the sub-message's field SET is
compile-time-known, so its WORST-CASE size is static, but its actual size is
runtime (varints shrink with their values). Of the honest mechanisms —
two-pass staging, an overlong fixed-width length varint (rejected: our own
decoder's overlong check refuses non-minimal varints), or back-patching the
length after the fact (rejected: a runtime byte-distance plus a shifting
rewrite) — the encoder takes two-pass staging, and it reuses machinery that
already existed. The compiler reserves a scratch region in the runtime frame
shaped as a `{ptr, len}` text descriptor followed by a staging buffer sized
to the largest nested child's worst case; the nested field encodes by
pointing the descriptor at the staging buffer, zeroing the len slot (which
doubles as the staging cursor), appending the child's fields into the buffer
with the ordinary wire appends, and then replaying the descriptor through the
same text-bytes append a String field uses — which emits exactly a length
varint followed by that many bytes. The parent's worst-case budget counts the
nested field as tag + length varint + child worst case, so the capacity rule
composes; nested fields are statically bounded, so the one-String-LAST rule
is unaffected by them and applies PER MESSAGE SCOPE (a child body simply has
no String today).

The decoder reads the nested tag, reads the length varint into the scratch
slot, then OPENS the sub-region: the length must fit the remaining buffer
(checked both as a raw value and as the absolute end bound, so a huge length
cannot wrap the 64-bit sum back inside the buffer), failure clearing the
sticky `ok` as usual. The child's fields then decode with the ordinary
expected-tag and value-varint reads — still bounds-checked against the full
buffer for memory safety — and a CLOSE check fails `ok` unless the cursor
landed EXACTLY on the declared end: a length that disagrees with the content
in either direction is a malformed payload, not a silent skew.

## Compatibility Reports

The compiler should be able to report protocol compatibility changes.

Example artifact shape:

```text
data CounterMessage:
  compatible:
    added field 3 timestamp_millis
  incompatible:
    field 1 changed i32 -> AtomicI32 without decode rule
  retired:
    field 2 retired in v2
```

This fits Omega's broader design direction: facts, obligations, and boundary
should be visible in build artifacts instead of hiding inside implementation
details.

## Working Rules

- Field numbers are stable protocol identities; declaration order is not.
- Identity is optional, stated, and never derived from order.
- Wire layout is not assumed to match runtime layout; grammars are layout
  policies chosen at use sites.
- Encoding and decoding are generated from source-visible contracts
  (plan-derived); hand-written byte code forfeits the conformance theorem and
  says so.
- Durability is a plan grade consumed at build time by longevity-contract APIs,
  never a fact about bytes.
- Unknown-field behavior must be explicit.
- Additions, removals, renames, type changes, and presence changes have
  explicit compatibility rules.
- Wire versions are allowed, but ordinary field evolution should be preferred
  when it is sufficient.
- Decode compatibility and runtime hot-swap migration are related ideas with
  different obligations.

## Open Design Questions

- Which grammars should ship after `compact_binary` (canonical text? packed
  repeated fields?), and how much of protobuf's ecosystem behavior
  (unknown-field preservation) is worth carrying?
- How should optional, required, repeated, and defaulted fields be spelled?
- How much compatibility can the compiler infer safely?
- When does a field type change require a new field number instead of a
  decode rule?
- The publish-time predecessor diff: exactly where it runs (package manager,
  build artifact comparison) and what it blocks.
- Surface migration of the implemented slice: legacy `wire data` form and
  `reserved` spelling → `data` + numbers + `retired` (mechanical; tracked in
  TASKS).

# Chapter 20: Wire Protocols

Omega should treat protocol schemas as first-class source-visible contracts.

Wire protocols are about stable external representation: bytes on a socket,
messages on a bus, persisted packets, RPC payloads, logs, and cross-version
communication with peers outside the current process image. They are not the
same thing as runtime data layout.

The design goal is protobuf-like: field identity is explicit, compatibility is
auditable, encoders and decoders can be generated, and protocol changes produce
compiler-visible facts.

## Wire Data

`wire data` declares an external schema with explicit field numbers.

```omega
wire data CounterMessage {
    0: counter: i32;
    1: timestamp_millis: i64;
}
```

Working interpretation:

- Field numbers are part of the wire contract.
- Field declaration order is not the authority; field numbers are.
- Renaming a field does not change its wire identity.
- Reusing a retired field number is illegal unless a protocol rule explicitly
  allows it.
- Decoding produces typed Omega values only after validation succeeds.
- Unknown-field handling is a protocol policy, not an accident.

This is closer to a schema language than an in-memory struct. The compiler
should be able to generate encoders, decoders, compatibility reports, and
protocol-diff artifacts from `wire data`.

## Runtime Shape Is Separate

Runtime `data` may be optimized for execution.

```omega
data Counter {
    counter: AtomicI32;
    timestamp: DateTime;
}
```

Wire data may be optimized for compatibility.

```omega
wire data CounterMessage {
    0: counter: i32;
    1: timestamp_millis: i64;
}
```

Conversion should be explicit, and ordinary machines should describe the
runtime transform.

```omega
trait WireWritable<Message> {
    machine Self::to_wire(&self, out: &mut Message);
}

trait WireReadable<Message> {
    machine Self::from_wire(message: Message, out: &mut Self);
}

machine Counter::to_wire(
    &self,
    out: &mut CounterMessage
) satisfies WireWritable<CounterMessage> {
    out.counter = self.counter.load();
    out.timestamp_millis = self.timestamp.to_unix_millis();
}

machine Counter::from_wire(
    message: CounterMessage,
    out: &mut Counter
) satisfies WireReadable<CounterMessage> {
    out.counter = AtomicI32::new(message.counter);
    out.timestamp = DateTime::from_unix_millis(message.timestamp_millis);
}
```

The exact trait names are provisional. The important rule is that protocol
boundaries are typed, generated, and auditable instead of hidden in hand-written
byte code. Omega does not need special `encode` or `decode` keywords just to
name transform code.

## Compatibility Rules

Wire schemas need explicit evolution rules.

The safe path should be easy:

- Adding an optional field is usually compatible.
- Removing a field should reserve the old number. When a declared `version`
  era documents the field, this is enforced: retiring a documented number
  without reserving it is a compile error. A schema with no version blocks
  is checked only for self-contained rules (duplicate tags, reserved reuse)
  -- the compiler cannot know undeclared history.
- Compatibility checks run along the VERSION CHAIN: each declared era is
  checked against its successor (v1 against v2, the newest era against the
  current body), matching how migrations compose in
  [Versioned Data](chapter_21_versioned_data.md).
- Renaming a field is compatible if the field number and meaning stay stable.
- Changing a field type requires an explicit compatibility rule.
- Changing requiredness or presence semantics is a protocol change.
- Changing defaults is a compatibility question, not just an implementation
  detail.
- Unknown fields may be rejected, ignored, or preserved depending on the schema
  policy. The same policy question covers unknown CASE TAGS on case-bearing
  wire fields (a newer era's case arriving at an older reader): reject,
  preserve raw, or decode as the zero case. In-language match exhaustiveness
  is never weakened for this -- cross-binary openness is a wire decode
  policy, not a type-system property.

Declared versions change the reservation story. Generated encodings always
carry an ERA DISCRIMINATOR (one varint per top-level message or record --
never per struct, never in native layout); a schema with no version blocks
encodes era `0`, and introducing versioning later snapshots the old body as
that era, so pre-versioning data stays decodable. Because the decoder always
knows a message's era:

- `reserved` protects against ACCIDENTAL reuse within an era; deliberate
  cross-era recycling of a retired number is legal -- the era tables
  disambiguate what proto must treat as radioactive forever.
- A field number changing type ACROSS eras is legitimate evolution, reported
  as "requires migration" (decode via the old era's table, migrate up the
  chain), not a compile error. Hard errors are for within-era violations and
  declared-history contradictions.

Example:

```omega
wire data CounterMessage {
    0: counter: i32;
    1: timestamp_millis: i64;

    reserved 3;
}
```

Reserved field numbers prevent accidental reuse after a field is retired.

## Wire Versions

Wire protocols can have explicit versions, but versioning should not be the
default answer to every schema change.

Most protobuf-style evolution should happen through field compatibility:
additive optional fields, reserved retired fields, stable tags, and explicit
decode rules. A version block is useful when the schema really has a distinct
era that cannot be described cleanly as ordinary field evolution.

Sketch:

```omega
wire data CounterMessage {
    version v1 {
        0: counter: i32;
    }

    reserved 0;
    1: counter: i32;
    2: timestamp_millis: i64;
}
```

This says old payloads used field `0`, while the current schema retired that
number and introduced the current fields.

Version blocks are potentially useful for:

- Breaking protocol eras.
- Compatibility with an already-shipped external format.
- Protocol envelopes that carry an explicit version.
- Decode paths where old payloads need different validation.

They are not a replacement for stable field numbering. If every field change
creates a new wire version, the protocol becomes harder to evolve and harder to
interoperate with.

## Decode And Runtime Conversion

Wire decoding and runtime migration are different operations.

Decoding bytes into a valid `wire data` value can be generated from the schema
and encoding family:

```text
bytes -> CounterMessage
```

Runtime conversion from that wire value into runtime data should still be an
ordinary machine:

```omega
machine Counter::from_wire(
    message: CounterMessage,
    out: &mut Counter
) satisfies WireReadable<CounterMessage> {
    out.counter = AtomicI32::new(message.counter);
    out.timestamp = DateTime::from_unix_millis(message.timestamp_millis);
}
```

If old wire versions exist, they can have conversion machines into the current
runtime shape.

```omega
machine Counter::from_wire_v1(
    message: CounterMessage::v1,
    out: &mut Counter
) satisfies WireReadable<CounterMessage::v1> {
    out.counter = AtomicI32::new(message.counter);
    out.timestamp = DateTime::unix_epoch();
}
```

This looks migration-like, but the obligations are protocol obligations:
validation, compatibility, unknown fields, canonical encoding, and external
stability. Runtime hot-swap migration has a different set of obligations and is
covered in the previous chapter.

## Encoding Families

A `wire data` declaration may eventually need to name an encoding family.

Sketch:

```omega
wire data CounterMessage encoding compact_binary {
    0: counter: i32;
    1: timestamp_millis: i64;
}
```

Possible encoding families:

- Compact binary with varints.
- Fixed-width binary.
- Canonical JSON-like text.
- Host-defined or domain-defined protocol encodings.

The encoding family determines low-level facts such as integer encoding,
endianness, field ordering requirements, packed repeated fields, unknown-field
preservation, and canonicalization.

The first implemented family is `compact_binary` v0, the default the
synthesized `Schema::encode_wire(&value, &mut out, &mut written)` encoder
emits for primitive integer fields (i32, i64, u32, u64, bool): the message's
ERA DISCRIMINATOR varint comes first, then each current-era field in
field-number order as a field-number varint followed by a value varint, where
varints are unsigned LEB128, signed values zigzag first
(`(n << 1) ^ (n >> 63)`, so small negatives stay short), and bool encodes as
one byte 0/1. The out buffer must be a `&mut [u8; N]` large enough for the
worst-case encoding (checked at compile time, so the encoder needs no runtime
bounds checks), and `written` receives the encoded byte count.

A `String` field rides as its tag varint, then a LENGTH varint (byte count),
then the raw UTF-8 bytes -- no NUL terminator, no padding. String fields are
ENCODE-ONLY today, and the encoder takes at most one per message, carrying
the schema's highest field number so it encodes LAST. Both restrictions fall
out of the same fact: a String's byte count is runtime-sized (the value is a
`{ptr, len}` text descriptor), so it cannot participate in the compile-time
worst-case capacity check. The worst-case budget covers everything up to and
including the length varint (ten bytes max); the trailing byte-copy is the
one append that bounds every store against the buffer's compile-time length
at runtime, DROPPING content past capacity rather than writing out of bounds
(callers size buffers for their longest expected text -- a runtime overflow
signal for encode is future work). Decode REJECTS String fields for now.
The honest storage options were: (a) zero-copy -- write a descriptor pointing
INTO the decode buffer, which makes the decoded message silently alias the
buffer; today's borrow facts track view loans created by explicit slice/text
borrow expressions only, so the checker CANNOT see a call output retaining a
borrow of another argument, and mutating or reusing the buffer would
invalidate the decoded string with no diagnostic; or (b) reject decode until
that aliasing relationship is checkable (or an allocator/copy target exists).
We took (b): encode-only is a smaller honest slice; zero-copy decode awaits
borrow facts that can model it (tracked in TASKS).

The matching decoder is
`Schema::decode_wire(&mut value, &buffer, &mut read, &mut ok)`: it reads the
era varint, then per field the expected field-number varint and a value
varint, un-zigzagging signed fields, and writes each value into the matching
field of `value`. `read` receives the byte count consumed and `ok` the
success flag. The decoder accepts the schema's CURRENT era only -- a payload
carrying any other era discriminator fails on its first byte; decoding
historical eras is deferred until the `Versioned<T>` container (chapter 21
stage 3) is signed off, since ordinary values cannot carry an era tag.
Failure semantics: `ok` is sticky -- the first violation (wrong era, a tag
that is not the next expected field number, truncated input, or an overlong
varint past ten groups) makes the decode report failure, and nothing can set
the flag back. On failure the decoder guarantees only the flag: `read` and
the message's fields may reflect a partial or garbage decode (no rollback),
but every byte read is bounds-checked against the buffer's compile-time
length, so a failed decode never reads out of bounds.

## Compatibility Reports

The compiler should be able to report protocol compatibility changes.

Example artifact shape:

```text
wire data CounterMessage:
  compatible:
    added optional field 2 timestamp_millis
  incompatible:
    field 0 changed i32 -> AtomicI32 without decode rule
  reserved:
    field 3 retired in v2
```

This fits Omega's broader design direction: facts, obligations, and boundary should
be visible in build artifacts instead of hiding inside implementation details.

## Working Rules

- Wire field numbers are stable protocol identities.
- Wire layout is not assumed to match runtime layout.
- Encoding and decoding are generated from source-visible contracts.
- Unknown-field behavior must be explicit.
- Additions, removals, renames, type changes, and presence changes have
  explicit compatibility rules.
- Wire versions are allowed, but ordinary field evolution should be preferred
  when it is sufficient.
- Decode compatibility and runtime hot-swap migration are related ideas, but
  they have different obligations.

## Open Design Questions

- Are wire field numbers always integers, or can protocols define custom tags?
- Which encoding families should Omega provide first?
- Should unknown fields be preserved by default for forward compatibility?
- How should optional, required, repeated, and defaulted fields be spelled?
- How much compatibility can the compiler infer safely?
- When does a field type change require a new field number instead of a decode
  rule?
- Should wire versions be explicit blocks, envelope-level metadata, or both?

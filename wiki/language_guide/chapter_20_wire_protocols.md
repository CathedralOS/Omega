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
- Removing a field should reserve the old number.
- Renaming a field is compatible if the field number and meaning stay stable.
- Changing a field type requires an explicit compatibility rule.
- Changing requiredness or presence semantics is a protocol change.
- Changing defaults is a compatibility question, not just an implementation
  detail.
- Unknown fields may be rejected, ignored, or preserved depending on the schema
  policy.

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

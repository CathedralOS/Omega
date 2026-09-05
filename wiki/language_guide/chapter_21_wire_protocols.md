# Chapter 21: Wire Protocols

Omega models a wire format as a checked interpretation of ordinary data.
Semantic values, byte grammar, historical migration, and deployment
compatibility each have one distinct owner:

```text
ordinary data declaration
        |
        v
reflected schema + selected codec policy
        |
        v
validated normalized plan
        |
        v
checked encoder and decoder realization
        |
        v
channel or storage compatibility policy
```

A single semantic type may use several codecs, and one codec may serve many
types. A runtime type may also have a separate message shape when its useful
in-memory representation contains atomics, handles, locks, or other values that
need an authored boundary conversion.

## Stable Member Identity

An identity-keyed schema assigns stable identities to fields:

```omega
data LoginMessage {
    #1 account_id: AccountId;
    #2 login: Text::LoginName;
    #3 nickname: Optional<Text>;
    retired #5;
}
```

The same rule applies to cases:

```omega
data Lookup<T> {
    case #0 Missing;
    case #1 Present(value: T);
    case #2 Redacted;
    retired #3;
}
```

The `#N` value is a schema identity. Identity-keyed codecs use it to recognize
the same member across declaration edits. Names remain source-facing, so a
rename preserves identity. A `retired #N;` entry reserves an identity that once
belonged to a field or case.

Identities are nonnegative `u64` values, unique within their scope, and distinct
from every retired identity in that scope. Their numeric order does not control
source matching or runtime placement; a codec may choose increasing identity
order as its canonical byte grammar.

Numbering is all-or-nothing within each member scope:

- fields of one record;
- cases of one sum; and
- fields of one structured case payload.

`retired #N;` places its scope in numbered mode as well. A policy that requires
stable identities accepts a fully numbered scope. Ordinary unnumbered data
remains useful for transient values and policies whose identity is positional.

Structured case payloads retain the ordinary parenthesized spelling:

```omega
data Request {
    case #1 Login(
        #1 account: AccountId,
        #2 source: LoginSource
    );
    case #2 Logout(#1 account: AccountId);
}
```

Stable identities are schema facts. A selected layout independently determines
offsets, alignment, tag width, and runtime discriminants.

An `[erased]` numbered field remains part of semantic and historical schema
identity, including compatibility checks and retirement history, but receives
no current codec placement and emits or consumes no tag or bytes. Decode
establishes its erased term through checked elaboration rather than raw input.
Erasure never renumbers the remaining wire identities.

## Presence Is Ordinary Data

Optionality is an ordinary generic sum:

```omega
data Optional<T> {
    case #0 None;
    case #1 Some(value: T);
}
```

`Optional<T>` is the standard spelling, while packages may declare other sums
whose cases better express their domain. Codec policies interpret the semantic
shape:

- a missing required field is invalid;
- an omitted `Optional<T>` field decodes as `None`;
- `Some(value)` encodes the field and its value; and
- omission is the canonical encoding of `None`.

Values required by a newer semantic shape are supplied by an authored migration
machine. This keeps meaning in checked program logic rather than in an implicit
wire default.

Repeated fields use ordinary sequence carriers:

| Carrier | Meaning |
|---|---|
| `[T; N]` | exactly `N` elements |
| `FixedVec<T, N>` | runtime length with statically bounded capacity |
| `Vec<T>` | growable owned sequence with allocation and resource obligations |
| `&[T]` | borrowed sequence |

The selected codec derives framing from the carrier and retains the applicable
length, allocation, and work obligations in its plan.

Implementation status: `compact_binary`'s generated realization now treats
`[T; N]` as exactly `N` elements and `FixedVec<T, N>` as a runtime length
bounded by `N`; neither uses an invented sibling count field. Borrowed byte
slices use the zero-copy length-delimited path. General borrowed scalar slices
are live for encoding: the normalized plan retains the runtime element count,
two scalar passes of work per live element, and the requirement that remaining
output capacity cover the canonical length prefix plus exact packed body. The
first pass measures that body and the second emits it without allocation.
Packed-varint decode cannot honestly produce a borrowed scalar view, so it
still requires an owned or caller-provided mutable destination carrier. Owned
`Vec<T>` remains gated on its explicit allocator contract.

## Representation Facts And Obligations

The home layout mechanically derives representation facts, including the
runtime discriminant of each case and the value denoted by all-zero storage.
Stable case identity does not choose that discriminant.

A representation-sensitive API publishes the relationship it relies on as an
ordinary machine requirement. The `Optional<T>` home-representation contract,
for example, includes:

```omega
machine zero_is_none<T>()
    ensures zero_value<Optional<T>>() == Optional::None;
```

The checker proves this obligation from the normalized home layout. A layout
change that gives all-zero storage another meaning fails where the
representation contract is declared.

This yields the general rule:

> Derived representation facts describe what the selected layout does.
> Authored representation obligations constrain what it is allowed to do.

Other layouts establish their own representation obligations. The wire rule
that omission decodes as `None` belongs to the codec contract and is independent
of the home-layout theorem.

## Codec Policies And Realizations

A codec policy consumes reflected schema and produces a plan from a closed
compiler-known placement vocabulary. The compiler validates and normalizes that
plan before it may drive encoding, decoding, or establishment.

The target-neutral placement derivation, authored-policy evaluation/agreement
gate, and encode-obligation recording live in `build-time-evaluation` over
Psi typed trees. Omega schedules that phase and consumes the checked plan during
target realization; it does not own the wire-language semantics.

The public operation family is expressed through ordinary requirements. The
encode half is:

```omega
trait Encode<Policy, Value> {
    machine encode(
        value: &Value,
        out: &write [u8],
        written: &mut u64
    );
}
```

Strict, projecting, and preserving decode are separate requirements below, so
unknown-member behavior is part of requirement identity. A codec's roundtrip
law relates its selected encode and decode requirements.

Realizations may be authored or generated from a validated plan. Origin and
trust are independent:

| Realization | Trust class |
|---|---|
| authored body checked against the requirement | derived |
| generated body independently checked against the requirement | derived |
| generator accepted as correct by construction | admitted, naming the compiler |
| opaque foreign codec | admitted, naming the provider |

The artifact retains the requirement identity, normalized plan identity,
realization origin, trust class, and supporting evidence. Differential tests
are valuable validation evidence for an admitted realization; a checked
contract is what promotes it to derived.

Implementation status: generated `compact_binary` report rows now retain
separate normalized `Encode<compact_binary, Value>` and
`StrictDecode<compact_binary, Value>` requirement identities plus the
normalized plan identity. Dynamic encode obligations are rendered with that
plan. Their realization origin is
`generated by Omega compiler compact_binary generator`, independently of their
trust class, `admitted by Omega compiler`. The evidence explicitly records
that the plan was validated and differential canaries ran, while the generated
body has not yet been independently checked against the public requirement.
Only that independent check may change the trust class to derived.

## Decoding And Unknown Members

Inbound bytes acquire semantic meaning through checked decode and
establishment. A codec package publishes the unknown-member behavior that each
decode operation provides:

- **strict** decoding rejects an unknown field or case and returns `T` only
  after validating the complete input;
- **projecting** decoding validates known members, discards unknown members, and
  returns `T`; and
- **preserving** decoding returns an ordinary package type such as
  `Relayed<T>`, containing the validated value plus an opaque remainder that can
  be emitted again.

Proofs about `T` range over the validated known value. The opaque remainder in
`Relayed<T>` carries bytes and ordering information needed for faithful relay,
without gaining semantic facts about fields the consumer does not know.

The standard package exposes these as the distinct `StrictDecode<Policy,
Value>`, `ProjectingDecode<Policy, Value>`, and `PreservingDecode<Policy,
Value>` requirements. `DecodeResult<T>` is the fail-closed result sum.
Preserving decode returns `DecodeResult<Relayed<T>>`; the carrier separates the
validated `value` from an `OpaqueWireRemainder` containing the producing codec
identity and one codec-private envelope holding exact unknown-member bytes plus
their relay ordering sidecar.
Opaque means semantically uninterpreted, not confidential or unforgeable.
Zero-copy packages retain an input-buffer loan in the remainder view; owned-copy
packages may publish an owned remainder carrier with explicit allocation and
resource obligations.

Zero-copy decoding returns a view whose loan is tied to the input buffer.
Owned-copy decoding chooses an owned carrier and therefore carries its
allocation and resource contract.

## Historical Formats And Migration

Published historical shapes are ordinary immutable declarations:

```omega
data CounterMessageV1 {
    #1 counter: i32;
}

data CounterMessageV2 {
    #1 counter: i64;
    #2 timestamp_millis: i64;
}

machine migrate_counter(
    old: CounterMessageV1
) -> CounterMessageV2;
```

A format package records the lineage between those declarations and selects
the decoder and migration machine for each accepted historical shape. Migration
is ordinary checked code, so changes of carrier, meaning, validation, and
required values remain explicit and auditable.

Domain-qualified fields make semantic evolution visible to the checker:

```omega
#2 login: Text::LoginName;
```

Changing that field to another semantic domain is a type change even if both
domains use the same carrier.

## Compatibility Belongs To The Edge

The channel, store, or package surface declares the histories it must
interoperate with. Typical policies include:

- exact schema identity for an atomically deployed internal channel;
- a bounded predecessor/successor window during a rolling deployment;
- a declared set of readable historical formats for persisted storage; and
- preserving decode for a relay that must round-trip fields it does not
  understand.

Build and deployment tooling compare the selected codecs, historical
declarations, migrations, and unknown-member behavior against that demand.
The resulting artifact reports:

- schema and codec identities;
- numbered fields and cases, including retired identities;
- historical shapes accepted by each decoder;
- migration routes to the current semantic shape;
- unknown-member behavior;
- canonicalization guarantees; and
- derived or admitted trust provenance for each realization.

A compatibility verdict is directional. Reading old data, being readable by an
old peer, preserving unknown information, producing canonical bytes, and
providing a complete migration route are separate facts. Each edge requests
the facts its deployment actually needs.

The final build declares each concrete channel/store demand:

```omega
machine build(builder: &mut Build) {
    builder.require_wire_compatibility<
        RollingChannel,
        CounterDisk,
        CounterMessageV2,
        CounterMessageV1,
        Readable,
        Writable,
        Canonical,
        CompleteMigration
    >();
}
```

The first four arguments name the edge, format lineage, local schema, and peer
schema. The remaining arguments are the requested facts from the closed
vocabulary `Readable`, `Writable`, `PreserveUnknown`, `Canonical`, and
`CompleteMigration`; omitted facts are still reported but do not reject the
build. `Readable` asks whether the local decoder accepts every peer value.
`Writable` asks the reverse question: whether the peer decoder accepts every
local value. `CompleteMigration` walks the selected
`FormatMigration<Lineage, Old, New>` conformances from peer to local.

The compiler writes every directional fact and its explanation to
`04_wire_protocols.txt`, then rejects an unsatisfied requested fact. With the
current generated `compact_binary` realization, canonicality is guaranteed and
unknown-member behavior is strict, so a `PreserveUnknown` demand fails until a
preserving codec package is selected.

## `compact_binary`

`compact_binary` is the first implemented Omega-native codec policy. Its
normalized tagged plan currently supports:

- numbered scalar fields using canonical unsigned LEB128;
- zigzag encoding for signed integers;
- canonical `bool` values `0` and `1`;
- bounded repeated scalar fields carried by exact arrays or `FixedVec`;
- trailing borrowed scalar slices through an allocation-free, exact two-pass
  packed-varint encoder;
- a trailing runtime-sized UTF-8 byte field;
- one level of length-delimited nested scalar records; and
- destination range checks before establishing qualified scalar fields.

The top-level frame starts with the policy's grammar discriminator, followed by
fields in increasing identity order. Nested records are length-delimited and
carry their own field sequence inside that bound. Every read is bounds-checked;
canonical varints use the fewest groups; nested decode must finish exactly at
the declared sub-region end.

Its current decode operation is strict. The first malformed value, unexpected
identity, truncation, range violation, or noncanonical encoding produces
`Invalid`. On failure, the verdict is authoritative while partially written
output fields and the consumed-byte count are unspecified. A `Sound` verdict
establishes every destination carrier and declared field domain.

The implementation generates encoder and decoder bodies from the normalized
plan. Until those generated bodies are independently checked against the public
codec requirement, the artifact classifies the realization as compiler-admitted.

Additional codec families, preservation containers, and channel policies build
on the same schema, plan, requirement, and artifact model.

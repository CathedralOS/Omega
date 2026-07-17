# Chapter 21: Protocol Schemas And Serialization

Omega has no separate `wire data` species and no first-class notion of a data
version. Protocol data is ordinary `data`. Serialization policies may consume
optional stable identity metadata when an external format must evolve without
silently changing what old bytes mean.

This chapter separates three things that are often conflated:

- a **semantic shape** is an ordinary `data` declaration;
- **identity metadata** says which protocol field or explicit discriminant a
  declaration member corresponds to; and
- a **layout/codec policy** says how values become bytes at a particular edge.

Only the second item is evolution-specific language metadata. It does not add
runtime state, change native layout, or make the value intrinsically
“versioned.”

## Stable Field Identity

Field identity numbers are optional metadata on plain `data`:

```omega
data CounterMessage {
    1: counter: i32;
    2: timestamp_millis: i64;
}

data Scratch {
    position: Vec2;
    zoom: f32;
}
```

The rules are:

- numbers may be sparse and appear in any declaration order;
- live and retired numbers must be unique;
- a number, not the source field name or declaration position, is the stable
  identity read by an identity-keyed codec;
- renaming a numbered field preserves that identity;
- numbers are inert to native layout, equality, proofs, ZII, and policies that
  deliberately use declaration order; and
- an unnumbered declaration has no durable field identity. A codec may not
  silently invent one from declaration order.

Identity numbers are therefore layout metadata for fluid external formats,
not a general-purpose versioning mechanism.

## Explicit Case Discriminants

Payload-less cases may pin the integer discriminant required by a foreign ABI
or protocol:

```omega
data MessageKind {
    case Invalid = 0;
    case Snapshot = 4;
    case Delta = 9;
}
```

As chapter 1 specifies, these values are likewise metadata consumed by an
appropriate layout policy. Internal sums leave discriminants implicit. A
protocol needing stable identities for payload-bearing alternatives must use a
codec/layout policy that states those tags or an ordinary explicit
discriminator field; Omega does not pretend the compiler's internal sum tag is
a durable protocol number.

## Retiring Identity

Deleting a published field burns its number:

```omega
data CounterMessage {
    1: counter: i32;
    retired 2;
    3: timestamp_millis: i64;
}
```

`retired N;` is metadata, not a ghost field. It does not participate in native
layout, reflection over live fields, equality, hashing, or ownership. Reusing
the number is a compile or publish error.

A single compilation cannot know undeclared history. Publication therefore
compares the normalized current schema/plan with its declared predecessor:
every old identity must still denote a compatible field or be explicitly
retired. Tombstones and predecessor comparison are complementary; neither is a
substitute for the other.

## Layout Policies Own Bytes

Encoding is selected at an edge. Owning a `CounterMessage` means owning a
value in compiler-sovereign native layout, not owning encoded bytes:

```omega
let value: CounterMessage;
let bytes: [u8; 128];
let written: count;

CounterCodec::encode(&value, &mut bytes, &mut written);
```

`OmegaLayout`, Protobuf, a file format, and a platform ABI are sibling layout
or codec policies over ordinary declarations. A policy may read identity
metadata, ignore it for an explicitly ephemeral encoding, or require
additional authored mapping. The declaration does not choose one universal
byte grammar for every use.

Layout plans normalize deterministically. The normalizer owns schema/layout
identity; proof strength may gate whether a plan or codec conformance is
accepted but may not change the published identity.

Encoding, decoding, and validation are ordinary contracted machines. The
compiler may provide a temporary hard-coded codec while build-time policy
machinery is incomplete, but arbitrary codec generation is not language
semantics. A complete codec conformance proves its declared round-trip,
validation, and compatibility laws.

## Compatible Evolution Within One Fluid Schema

An identity-keyed, self-delimiting format can support compatible changes to a
numbered declaration:

- add a field under a fresh identity when its absence has an honest declared
  meaning, normally ZII;
- rename a field while retaining its identity;
- retire a removed field identity; and
- apply an explicit unknown-field policy: reject, skip, or preserve opaque
  bytes for re-emission.

Changing a field's type, default interpretation, unit, or semantic meaning is
not made safe by retaining its number. The predecessor comparison must reject
structural incompatibility, while semantic compatibility remains an authored
contract and review responsibility.

These rules describe what a particular codec can preserve. Field numbering
does not itself force a tagged grammar or make every future change compatible.

## Breaking Eras Are Ordinary Shapes

A breaking format change gets a new, ordinary declaration:

```omega
data CounterDiskV1 {
    1: counter: i32;
}

data CounterDiskV2 {
    1: counter: i32;
    2: timestamp_millis: i64;
}

data DecodedCounter {
    case Invalid;
    case V1(value: CounterDiskV1);
    case V2(value: CounterDiskV2);
    case Unknown(raw: OpaqueFrame);
}
```

Here `OpaqueFrame` is a format-package-owned bounded byte carrier. The outer
format chooses how bytes identify an era: an ordinary numbered
field, an explicit protocol discriminant, negotiated framing, a database
schema table, or some foreign rule. The decoder applies that rule and returns
an ordinary closed sum over the eras this program understands. `Unknown` is
included only when the format promises opaque preservation; strict formats
reject instead.

The sum receives ordinary exhaustive matching and payload narrowing. Adding a
known era breaks consumers that enumerate cases, exactly as adding any other
sum case does. There is no separate version-match grammar.

Omega deliberately has none of the following language constructs:

- `version vN { ... }` blocks inside a declaration;
- compiler-generated historical types such as `Counter::v1`;
- a compiler-owned `Versioned<T>` container or `.era` query; or
- an implicit era discriminator prepended to every encoded message.

Different histories may target the same runtime type: disk, network, cache,
and live-component state can each use independent shapes and policies. That is
the primary benefit of not attaching one privileged lineage to a runtime
declaration.

## Runtime Shapes Stay Separate

Runtime representation and durable representation usually want different
types:

```omega
data Counter {
    counter: AtomicI32;
    timestamp: DateTime;
}

data CounterDiskV2 {
    1: counter: i32;
    2: timestamp_millis: i64;
}
```

Conversion between them is ordinary machine code with ordinary effects,
ownership, failure, and invariant contracts. A generator may transcribe a
codec for an explicitly authored protocol shape; it may not decide that an
atomic cell, capability, cache, clock value, or other runtime field has a
particular durable meaning.

## Durability Is A Plan/API Property

The resolved layout plan can be graded for durability:

- a durable storage API requires stable identity and the reader-tolerance
  properties its contract promises;
- a same-build cache may choose a compact positional layout and carry a
  schema fingerprint so stale bytes reject deterministically; and
- a foreign ABI layout is pinned by its external specification rather than by
  Omega's schema-evolution rules.

Durability is not a domain attached to owned values or arbitrary bytes. It is
a checked relationship among a schema, a codec plan, an API promise, and—when
published—its predecessor artifact.

## Current Compiler Bridge

The current `compact_binary` implementation is a bootstrap bridge, not the
final semantic surface. It supports a restricted identity-keyed grammar,
primitive scalar fields, limited strings and nesting, and byte-exact
interpreter/native canaries.

It also still implements legacy versioning machinery: `version` blocks,
`Versioned<T>`, version-match arms, and an unconditional leading era value.
Those features are scheduled for removal in `TASKS.md`. Their canaries describe
the implementation being retired, not source compatibility promised by the
language. New external formats must not publish that transitional era prefix as
a stable Omega guarantee.

## Compatibility Reports

Build artifacts should expose normalized, typed identities and compatibility
results rather than a single human “version” string:

```text
schema CounterMessage
  schema identity: ...
  codec-plan identity: ...
  compatible: added field 3
  retired: field 2
  incompatible: field 1 changed i32 -> AtomicI32
```

Schema identity, codec-plan identity, component contract identity, and provider
identity are distinct types even when they share hashing infrastructure.
Compatibility and refinement certificates connect identities; equality does
not replace those relations.

## Working Rules

- Protocol schemas are ordinary `data`.
- Stable field numbers, explicit discriminants, and tombstones are inert
  layout/serialization metadata.
- Layout and codec policies own byte grammar at use sites.
- Compatible fluid evolution is checked against a predecessor plan.
- Breaking histories use explicit named shapes, ordinary sums, and ordinary
  migration machines.
- Open-world input is handled at decode boundaries by an explicit unknown-era
  policy.
- Runtime types never acquire one privileged format lineage.

## Still Open

- the exact publish-time predecessor selection and certificate format;
- unknown-field/case opaque-preservation storage in the first general codec;
- stable tag metadata for payload-bearing protocol alternatives;
- version negotiation and explicit downgrade protocols; and
- the final programmable-layout reflection and plan vocabulary.

# Codecs and format policies

Encoding, decoding, and validation are ordinary checked library-machine
requirements. A format package publishes its selected [layout plan](plans.md),
codec requirements, realizations, and trust evidence. Bodies may be authored
or generated from a validated plan; neither origin bypasses conformance checks.

## Agreement and trust

A concrete codec conformance proves its public agreement requirements,
including the current-shape law `decode(encode(value)) == value` where required
by the codec contract. Historical migration belongs to the format lineage and
composes outside that law.

| Realization | Trust classification |
| --- | --- |
| Authored or generated body independently checked against the public requirement | Derived. |
| Generator accepted as correct by construction | Admitted, naming the compiler as trusted party. |
| Opaque foreign realization | Admitted, naming its provider. |

Artifacts retain normalized plan, requirement identity, realization origin,
trust class, and evidence independently. Generated origin does not mean derived
trust. Independent generated-codec verification and preserving-codec
realizations remain implementation work.

## Boundary establishment

Inbound code receives raw bytes or inert pointers under a boundary contract,
validates or materializes the selected representation, establishes proven
predicates and any separately authorized qualification, then exposes Omega
values or checked borrowed views. A validator may establish a predicate only
because its checked contract proves it; user code cannot construct a supposedly
valid payload without that proof. Decoding alone establishes no trust,
authority, or device-correspondence fact.

Outbound code forgets facts or executes an explicit encoding/conversion before
crossing the boundary. A provider's foreign representation does not change
ordinary application types. [Recasts](recasts.md) preserve representation;
they cannot fabricate stronger validity or replace executable conversion.

## Policy selection and durability

A bare codec call may use the exact policy already named by the destination's
declared policy domain. Otherwise the call names the policy explicitly.
Adding an unrelated import or conformance cannot change candidate meaning;
third-party conformances are callable by name, not visible-conformance search.
Selection through generics remains implementation work.

Durability and self-description are properties of the normalized plan and its
consuming API contract, not semantic domains on arbitrary bytes. A durable
store may require stable identity and reader-tolerance guarantees. An ephemeral
cache may explicitly select a policy without them. Channel/store compatibility
checks must join published schemas, plans, historical shapes, and migrations;
they are not implied by one codec round-trip law.

## Presence and representation

A missing required field is invalid. An omitted `Optional<T>` field decodes
as `None`; omission is its canonical encoding, while `Some(value)` encodes the
field and value. New required semantic values need an authored migration, not
an implicit wire default. Exact arrays contain exactly their declared count;
bounded-live-length, growable owned, and borrowed sequences retain their own
length, allocation, loan, and work obligations.

Home-layout facts describe runtime discriminants and the meaning of zero
storage; stable case identities do not select either. A representation-sensitive
API states the relationship it requires as an ordinary checked obligation.
The `Optional<T>` home-representation obligation that zero denotes `None` is
separate from the codec's omission rule. A layout that violates an authored
representation obligation rejects where that obligation is declared.

## Unknown members and remainder custody

`StrictDecode<Policy, Value>` validates the complete input and rejects unknown
fields or cases. `ProjectingDecode<Policy, Value>` validates known members and
discards unknown ones. `PreservingDecode<Policy, Value>` preserves them for relay.
These are distinct normalized requirements, not an unrecorded decoder option.
`DecodeResult<T>` is the fail-closed result sum; preserving decode returns
`DecodeResult<Relayed<T>>`.

`Relayed<T>` separates its validated value from an `OpaqueWireRemainder` binding
the producing codec identity and a codec-private envelope containing exact
unknown-member bytes and their relay-ordering sidecar. Facts about `T` describe
only the known validated value. Opaque means semantically uninterpreted, not
confidential or unforgeable. A zero-copy remainder or decoded view retains an
input-buffer loan; an owned copy carries explicit allocation and resource
obligations. Packed-varint decoding cannot substitute a borrowed scalar view
for the required owned or caller-provided mutable destination.

## Historical lineages and migration

Published historical shapes are immutable ordinary declarations. Independent
lineages may share carriers without sharing migration edges. The standard
`FormatMigration<Lineage, Old, New>::migrate` requirement selects an explicitly
bound checked conversion for the exact lineage and shapes. It adds no intrinsic
version identity to either type. Reverse and fallible conversions are separate
requirements; an upgrade promises neither reversibility nor a downgrade.
Changing a field's semantic domain is a type change even with the same carrier.

Decode policy handles unknown eras explicitly: reject, preserve, negotiate, or
another contracted choice. Exhaustive matching checks known cases, but a wildcard
does not prove that every known era has a migration route. Such completeness
needs the selected migration evidence. Generators may traverse schemas; they
do not choose persistent fields, atomic snapshots, or migration meaning.

## Compact binary policy

The `compact_binary` grammar starts with its policy discriminator and emits
fields in increasing stable-identity order. Integers use canonical minimal
unsigned LEB128 groups, with zigzag for signed values; Boolean encodings are
exactly `0` and `1`. Nested records are length-delimited and their decoding must
finish exactly at the declared sub-region end. Reads are bounds-checked.

Its strict decoder returns `Invalid` on malformed values, unexpected identities,
truncation, range violations, or noncanonical encoding. That failure verdict is
authoritative; partially written output fields and consumed-byte count are
unspecified. `Sound` establishes every destination carrier and declared field
domain. Generated origin does not independently prove this contract; an
unverified generated realization remains compiler-admitted.

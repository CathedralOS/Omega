# Codecs and format policies

Encoding, decoding, and validation are ordinary checked library-machine
requirements. A format package publishes its selected [layout plan](plans.md),
codec requirements, realizations, and trust evidence. Bodies may be authored
or generated from a validated plan; neither origin bypasses conformance checks.

Named decision: `standard-fallible-representation-conversions`, settled
2026-10-27. Core defines the common conversion requirements; a representation
package supplies their explicitly selected ordinary implementations. A C
policy may ship in core or std; protobuf and other formats may be independently
authored packages. Their names are not compiler built-ins. Fixed-memory layout
and variable-length wire encoding have distinct consuming capabilities even
when both expose a library family such as `CLayout<T>` or `ProtoLayout<T>`.

## Representation conversions

The standard ordinary trait `TryFrom<Source, Destination, Error>` supplies a
checked conversion returning `Result<Destination, Error>`. Both endpoints are
explicit complete types; there is no implicit `Self`, data subject or search
for a visible implementation. Full-conversion representation families provide
selected conversions to and from their semantic T under their stated resource
and value preconditions. To/Into-style helpers delegate to that same selected
implementation rather than defining a competing conversion map.

The standard ordinary `Representation` trait groups those exact selected
conversion applications and their agreement laws, with error and resource
contracts explicit. A named implementation supplies this map; the storage data
type does not itself satisfy the trait. Fixed geometry alone does not satisfy
Representation or authorize a decoder. Generic callers require the selected
map and its public guarantees instead of inferring conversion from a layout
name or the existence of a plan machine.

Buffers, allocator state, work sponsorship and output custody are explicit
inputs. A buffered or allocating conversion carries them in its explicit
source request or uses the corresponding explicitly parameterized codec
requirement; the simple source-to-result form does not grant an ambient
allocator, hidden capture environment or host-service access. Failure retains
or disposes of borrowed/owned inputs under the declared custody contract and
never exposes an unestablished destination as a valid value. In-place failure
and partially written output obey the particular codec's documented contract.

The common interface is fallible because a generic caller cannot assume
success. A selected conversion may additionally expose an infallible `From`
convenience only when totality is established for every admitted input under
its complete preconditions. It delegates to the same implementation; the
compiler may eliminate a proved-unreachable error path, never assume one away.
There is no silent layout coercion at a call or foreign boundary.

A full conversion contract requires actual coverage, not just signatures:
supported values encode successfully with sufficient declared resources; valid
representations decode to established values; malformed or nonrepresentable
inputs fail; and the selected round-trip or preservation law holds. Returning
Unsupported for every input does not implement such a contract. Geometry-only,
sparse, read-only and write-only policies remain usable under their actual
capabilities without claiming complete bidirectional conversion.

The representation author supplies generic reading/materialization and
writing/encoding rules once. [Typed reflection](../language/reflection.md)
and ordinary specialization elaborate the authorized fields, active cases and
exact callback selections for each supported T. Cross-representation adapters
compose through semantic T instead of requiring a separately authored adapter
for every pair. Optimizations may remove intermediate storage only while
preserving behavior, custody, effects and failure contracts. Unsupported members
reject with their exact field/case and missing operation; no member is silently
skipped. Semantic field changes, historical migrations and custom normalization
still require explicit selected rules. Generated source text, macros, a
format-specific derive operation and spelling-based proof rules are not the
mechanism, and specialization alone establishes no new trust classification.

Raw bytes are not automatically an established T. A representation carrier's
physical geometry and its current value-validity facts remain separate;
checked construction/conversion establishes the latter before an API relies
on them. Exclusive byte writes and deferred establishment use the existing
invariant-window and restoration rules. Layout names mint neither backing nor
authority. Variable-length representations need explicit encoded extent and
owned or borrowed storage, with their ordinary lifetime and allocation
obligations; they do not become fixed-size or addressable C records.

If ProtoLayout names protobuf, its library must implement and document protobuf's
actual field-number, wire-type, presence, unknown-member and compatibility rules.
The compact-binary policy below is a different format, not a protobuf synonym.
No arbitrary community format is installed or implemented by this ratification.

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
trust. `Derived` requires independently checked evidence establishing the public
requirement for every input covered by its contract, tied to the exact codec
realization and schema. Differential probes over selected values and malformed
frames are regression checks, not a general agreement argument. Reporting their
finite scope does not qualify the realization as `Derived`. Without sufficient
agreement evidence, a generated realization remains `Admitted`, naming the
compiler as the trusted party. Preserving-codec realizations remain
implementation work.

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

A codec call uses the exact implementation already selected by its
representation application or names the implementation explicitly. Domain
qualification does not choose storage geometry or encoding policy.
Adding an unrelated import or conformance cannot change candidate meaning;
third-party conformances are callable by name, not visible-conformance search.
Generic selection, reflection specialization and library-authored realizations
remain implementation work; the requirements above are the accepted contract.

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

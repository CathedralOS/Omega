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

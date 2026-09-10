# Terminal canonical encoding and identities

[Portable product](product.md) | [Observations](observations.md)

This is the canonical encoding contract, not yet a complete byte-level decoder
specification. The [documentation index](../../README.md) lists operation subjects.
[Proof values](mathematical_values.md) and [certificate rules](integer_certificates.md)
define semantics; complete per-operation and proof-node physical tables still
need specification under PSIIR on the [execution board](../../../TASKS.md).
The implementation's codec is not a substitute for those tables.

## Canonical form

Semantic bytes begin with `PSITERM\0`. They use fixed-width little-endian counts,
stable nonzero identities, full-width integer payloads, and explicit closed-sum
tags. Unordered sets are strictly sorted by stable identity or canonical bytes
and reject duplicates; symmetric terms use the same ordering. Parameters,
operations, jump arguments, and other execution-significant sequences retain
their declared order. Recursive terms have a fixed supported depth bound.

Decode rejects stale markers, unknown tags, invalid identities/values,
noncanonical order/forms, verifier-invalid modules, truncation, and trailing
bytes. Successful decoding re-encodes byte-for-byte; it never normalizes an
alternative spelling. The pre-release contract has no implicit migration:
semantic changes move producer, codec, verifier, interpreter, and lowerers
together. Current golden bytes establish the current format, not historical
acceptance.

Operation variants are closed and typed. Their referenced values must be
available under the operation's definition/dominance rules. Each reconstructs
its logical result and obligations; an encoded proof cannot select them.

Scalar declarations encode a qualification-set identity after their payload
type. Zero denotes the bare set and has no catalog row. The module's scalar
qualification catalog follows the entry identity: counted domain definitions,
counted normalized sets, then counted introduction edges. Definitions bind their
local and semantic identities, canonical theory identity and scalar carrier.
Sets are nonempty, contain strictly ordered unique domain identities, and have
unique nonzero identities; duplicate sets reject. Introduction rows retain
machine, edge, argument ordinal, source and destination identities, ordered
strictly by `(machine, edge, argument ordinal)`. All these bytes participate in
semantic identity. Canonical encoding does not itself authorize membership:
verification checks the exact arrival and same-carrier addition under the
[scalar qualification rules](calls_and_outcomes.md#scalar-qualifications).

The semantic scalar-range-invariant roster precedes the machine table. Rows
are strictly ordered by `(machine, header, parameter)` and encode those three
identities, the integer carrier, inclusive minimum and maximum, and a counted
arrival list. Arrivals are strictly ordered by edge identity and retain the
edge and obligation identities. Every actual header arrival must occur exactly
once. Bounds and the roster participate in semantic identity; their certificates
remain replaceable flat proof evidence. The corresponding reconstructed ledger
owner retains machine, header, parameter, and edge independently of evidence.

## Residual jump encoding

Within the terminator tag space, a Jump with no residual affine discards uses
tag 1; a nonempty residual list uses tag 10. Both have the same jump semantics.
Tag 10 with an empty list is noncanonical. Each residual retains place, ordered
structural path, and exact subtree type; ownership validation reconstructs the
complement rather than trusting this list.

## Section identities

| Section | Identity and role |
| --- | --- |
| Semantic module | Domain-separated commitment to exact canonical bytes; excludes replaceable proof, installation, and debug sections. |
| Proof bundle | Separate `PSIPRF\0\0` bytes and identity, binding current proof-system/format markers and exact evidence. |
| Optimization execution | Separate evidence binding executed selection and input/output semantic/proof identities, rejoined at decoding. |
| Installation | Separate `PSIINST\0` bytes and identity, retaining realization evidence without granting admission. |
| Debug map | Replaceable presentation metadata bound to the exact semantic subject, never program meaning. |

The reconstructed manifest binds each section under its own hash domain.
Absent differs from present-but-empty. Replacing valid nonsemantic evidence
preserves semantic identity while changing its own section and container
identities.

Proof evidence is strictly ordered by obligation identity and retains exact
rules, proof trees, and admissions. Preserve cited rule direction even though
proof bytes are replaceable. Disjunction introduction retains one checked child
and its selected canonical arm; absent/out-of-range arms and a child concluding
another arm reject. A proof-calculus constructor does not itself authorize a
semantic-ledger rule or an unproved reduction procedure.

## Retained placement custody

A source-derived placed-view input binds Terminal machine/state/parameter
coordinates to hermetic source, policy, producing-plan-machine, and schema
identities; the policy/schema-derived view identity; exact access and binding
modes; a report-only compact coordinate; and the validated plan's
domain-separated layout/access/reach commitment.

Reject missing machines, owned access in this view role, non-hermetic identities,
zero required report coordinates or commitments, duplicates, and noncanonical
order. The row grants no runtime storage, accessor, provider, physical address,
or ABI authority.

## Installation and semantic-code attribution

Installation bytes bind semantic identity, target facts, exact profile/provider
decisions, complete emitted-image hash, and text-validation evidence. Retain
separate domain-framed strong digests for encoded compiler text, final compiler
text, canonical relocations, and their derivation. Compact fingerprints are
report compatibility, not substitutes for those commitments. Installation still
consumes separate admission and placement authority; decoding yields an audit
projection, not an executable grant.

Effectful roots additionally retain the canonical function map, each privileged
port effect's service/operation/byte range, and each boundary settlement's exact
admitted execution binding and associated preceding realization. A settlement
emits no duplicate hardware effect. Reject missing, reordered, byte-drifted, or
raw-number-only realization evidence. Production uses the same admitted
provider executions as lowering and checks the complete emitted settlement
closure.

Emitted operations and return edges retain semantic site, operation ordinal,
function-relative offset, and byte count. Metadata-only settlement rows have
zero-byte intervals. This is replay/analysis provenance, not native instruction
cost or runtime charging. Structural call placement must preserve the
[borrow identity contract](structural_access.md);
it cannot stage every borrowed parameter as an owned copy merely because the
installation record can describe one.

# Psi wire format

The wire codec turns a [Psi program](../../../../../../wiki/spec/terminal-psi/product.md) into bytes and checks the structure of a program it reads. The [encoding specification](encoding.md) defines the canonical form and identifies each section. The individual operation payloads currently live in the vocabulary reference.

This crate reads and writes the complete program, validates incoming records, computes fingerprints and handles companion sections. The [artifact codec](../codec/README.md) seals each program with its bytes and manifest. The optimizer and native structural catalog get the bytes for structural domains, access plans and quotient declarations directly from [declaration encoding](../declaration-encoding/src/lib.rs). Both the complete artifact and the native compiler use those same field definitions.

[lib.rs](src/lib.rs) exposes the public API. Start with the operation's file:

- [semantic_module.rs](src/semantic_module.rs) runs canonical ordering,
  representation checks, encoding, decoding and semantic identity.
  The `*_wire` files beside it handle individual payloads; [foundation_validation.rs](src/foundation_validation.rs)
  checks representation shape and [wire_fields.rs](src/wire_fields.rs) defines fields specific to complete programs.
- [closed_types.rs](src/closed_types.rs) records the closed integer types one
  complete encoding retains.

The [Psi product contract](../../../../../../wiki/spec/terminal-psi/product.md#publication-and-separate-consumption)
lets a host reuse checker verdicts when it trusts the producer of that exact
artifact. Canonical decoding shows valid encoding and structure, not true
claims or permission to run. The receiver need not rediscover theorems or
replay certificates.

The codec carries certificate bytes inside mathematical proof terms and indexed
theorem roots, including the `PSICORE` framing. Keep those canonical encodings;
they grant nothing at acceptance. The local proof kernel and the interpreter's
closed evaluation are separate from byte transport.

The [implementation plan](../../../../../../wiki/drafts/designs/psi_claim_admission.md)
separates canonical and structural checks from run-only preparation.
Today decoding also runs module-foundation and dynamic-result analyses,
including descriptor and entailment work. The split keeps structural
validation, moves run-only work to preparation and keeps exact reusable
results. Shared input and total-work limits must cover these paths; per-query
search caps alone cannot bound loading cost to the host's budget.

The other modules under [sections.rs](src/sections.rs) encode manifest, debug and optimization data.
The [shared encoding error](../declaration-encoding/src/error.rs) identifies failures from declaration fields and complete programs.
Do not infer one section's identity from another section's current version.

Current markers live with their users: semantic format in [semantic_module.rs](src/semantic_module.rs),
vocabulary in [VocabularyMarker](../../identities/program/identity.rs),
[canonical_artifact.rs](../codec/src/canonical_artifact.rs), and manifest in
[sections.rs](src/sections.rs). Installation section commitments are separate from
semantic identity and grant a receiver no installation rights.
The [module wire decoder](src/module_wire.rs) rejects incompatible markers.
Source-lowering tests check semantic round trips without copying a changing
wire-version number.

Scalar declarations encode their exact qualification-set identity. The semantic
module encodes the predicate- and route-free domain catalog, canonical
nonempty sets, and exact introduction/erasure-edge positions. Zero names the
bare set, not a missing old field. Changing any definition, value set or coercion changes semantic
identity; decoding never infers membership from the scalar payload.

Bounded integer fields keep their fixed carrier and both full-width inclusive
endpoints. Invalid carriers or endpoints, reversed bounds and erased bounded
fields reject. Their declaration bytes are part of semantic identity; the same
layout does not make different ranges interchangeable.

Integer and Boolean structural field reads encode the source root, the
counted canonical carrier path, and the final declaration-local field ID. An
empty path is a direct read; each step is a record field ID. Case and
fixed-index steps reject here. Every step is part of the semantic bytes and
proof equations; decoding never fills in a missing path.

Scalar case construction encodes the selected case and its complete field roster
in declaration order. Each field binds an already evaluated scalar value and, for a bounded integer
field, its declared range obligation. The case, operand and obligation
identities are all part of semantic identity; an empty field list is the
payloadless form of the same operation. Encoding proves nothing. Source
checking proves the range facts; receiver structural checks confirm the exact
match, and source verdicts are reused only when the host trusts the producer.

## Observation profile

No code builds or checks the
[TerminalTraceV1 observation profile](../../../../../../wiki/spec/terminal-psi/observations.md):
no stage derives the profile from a validated module, the encoder writes no
profile section and acceptance compares none. The earlier unused profile types
in `observation.rs` were deleted. **OBSERVATION-PROFILE** in
[TASKS.md](../../../../../../TASKS.md#observation-profile) tracks the deriver,
codec and acceptance comparison.

## Current-only decoding

The module decoder accepts only the current semantic format and vocabulary
markers. It rejects every other marker, including mismatched format/vocabulary pairs,
before reading the module body. Every current list is required; the decoder
does not fill in old rows or migrate older artifacts. Proof, envelope and
installation markers are versioned separately. Current certificate-bearing
forms are still carried and canonically decoded.

Decoding checks canonical form and structure, which is all inspection needs.
The lowered-form checker's judgments that execution relies on (structural
memberships, external-completion providers and dynamic view results) run in
`sealing::prepare_received_execution` instead, once, when the receiving host
authorizes a received module for execution.

Foundation validation still refuses a move or leaf copy whose result keeps a
domain its source no longer holds at that point. A call or store before the
transfer can take a grant away; `structural_qualifications::validate` compares
only each place's declared domains and does not see it.
`terminal_semantics::descriptor_results::validate_transfer_source_memberships`
replays the machine up to the transfer to check it.

Run the codec's tests with:

```sh
mbx test -p terminal-codec --lib < /dev/null
```

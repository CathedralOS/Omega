# Terminal codec

Public contract: [Terminal product](../../../../../wiki/spec/terminal-psi/product.md).
The [canonical encoding contract](../../../../../wiki/spec/terminal-psi/encoding.md)
owns canonicalization and section identity. Per-operation wire payloads remain
in the vocabulary reference while that part is consolidated.

[lib.rs](src/lib.rs) exposes the public API. Start with the operation's owner:

- [semantic_module.rs](src/sections/semantic_module.rs) sequences canonical
  ordering, representation checks, encoding, decoding, and semantic identity.
  Its `*_wire` children own individual payloads; `module_foundation_validation`
  checks representation shape and `wire` owns byte cursors.
- [canonical_artifact.rs](src/canonical_artifact.rs) binds semantic, proof,
  optimization, and optional debug bytes into the handoff envelope.
  [publication.rs](src/publication.rs) stages and publishes canonical semantic bytes.
- [artifact_admission.rs](src/artifact_admission.rs) checks receiver policy,
  independently replays proofs, and compares the complete reconstructed claim
  before issuing a qualified verdict. Canonical decoding alone does not admit proofs.
- [proof_sidecar.rs](src/sections/proof_sidecar.rs) owns sidecar claim encoding;
  [psi_claim.rs](src/sections/proof_sidecar/psi_claim.rs) reconstructs Psi profiles,
  guarantees, assumptions, and dependencies for producers and receivers.
- [trust_graph.rs](src/trust_graph.rs) owns the current implementation's trust
  closure, with construction, identity, and validation in its children.
  [verified_program.rs](src/verified_program.rs) projects program-local producer
  schemas from verifier-owned evidence. Neither is an encoded artifact section.

The other `sections/` owners retain independently identified proof, manifest,
debug, optimization, obligation-ledger, and observation-profile encodings.
[codec_error.rs](src/codec_error.rs) owns the shared encoding error vocabulary.
Do not infer one section's identity from another section's current version.

Current markers live with their owners: semantic format in [semantic_module.rs](src/sections/semantic_module.rs),
vocabulary in [VocabularyMarker](../src/terminal_module/identity/vocabulary.rs),
proof format in [proof_bundle.rs](src/sections/proof_bundle.rs), envelope in
[canonical_artifact.rs](src/canonical_artifact.rs), and manifest in
[artifact_manifest.rs](src/sections/artifact_manifest.rs). The image-emission owner
maintains the separate [installation format](../../../../omega/backend/images/image-emission/src/installation_record.rs).
Codec-owned [current-format tests](src/sections/semantic_module/current_format_tests.rs) pin canonical
bytes and incompatible-marker rejection. Source-lowering tests check semantic
round trips without duplicating a transient wire-version number.

Scalar declarations encode their exact qualification-set identity. The semantic
module encodes the predicate-/route-free domain catalog, canonical nonempty sets,
and exact introduction/erasure-edge coordinates. Zero names the bare set, not an omitted
historical field. Changing any definition, value set or coercion changes semantic
identity; decoding never infers membership from the scalar payload.

Bounded integer fields retain the fixed carrier and both full-width inclusive
endpoints. Invalid carrier/endpoints, reversed bounds, and erased bounded fields
reject. Their declaration bytes participate in semantic identity; an identical
physical layout does not make different numeric restrictions interchangeable.

Integer and Boolean structural field reads encode the source root, the counted
canonical carrier path, and the final declaration-local field ID. Empty paths
are direct reads; each nonempty step is a relevant record field ID. Case and
fixed-index steps reject in this operation family. Every step participates in
semantic bytes and proof equations; decoding never supplies a missing path.

Scalar case construction encodes the selected case and its complete field roster
in declaration order. Each field binds an already evaluated scalar value and,
for a bounded integer field, its declaration-derived range obligation. The case,
operand identities, and obligation identities all participate in semantic identity;
an empty roster is the payloadless case of the same operation. Encoding a module
does not establish its proofs. Artifact acceptance independently verifies the
range obligations before treating the constructed payload as valid.

## Observation-profile implementation

[terminal_trace_v1_profile.rs](src/sections/terminal_trace_v1_profile.rs) implements
canonical profile encoding and module-bound acceptance for
[TerminalTraceV1](../../../../../wiki/spec/terminal-psi/observations.md). Acceptance
validates the module, derives its complete identity and site roster independently,
and compares the decoded profile exactly. Revision 2
(`omega.terminal.observation-profile.v2`) appended the operation-crash group:
each `TrappingInteger` operation is its own `Trap` site, recorded with its
primitive and carriers rather than a fabricated edge or boundary route.

The current encoder writes the terminal-external group with zero count; the
decoder rejects a nonzero count. That is an implementation fence, not an assertion
that the semantic trace excludes successful external termination. The
[canonical ProcessExit contract](../../../../../wiki/spec/language/process_exit.md)
settles requirement-owned completion identity without a new source keyword.
The [source-to-verifier migration](../../../../../TASKS.md#process-exit-contract)
remains unimplemented; nonzero rows must continue to reject until that route
retains and independently verifies the exact terminal transfer.
Scalar and whole-root structural comparison helpers provide value comparison
only, not trace construction or refinement evidence.

## Current-only decoding

The module decoder accepts only the current semantic format and vocabulary
markers. It rejects every other marker before reading the module body, including
crossed format/vocabulary pairs. All current rosters are required; the decoder
does not supply missing historical rows or migrate an older artifact.
Proof, envelope, and installation markers remain independently owned.

Run the marker, captured-byte, truncation, and envelope-admission controls with:

```sh
cargo nextest run -p terminal-codec --lib --no-fail-fast --no-tests fail -E 'test(current_format_tests)'
```

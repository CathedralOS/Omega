# Terminal codec

Public contract: [Terminal product](../../../../wiki/spec/terminal-psi/product.md).
The [canonical encoding contract](../../../../wiki/spec/terminal-psi/encoding.md)
owns canonicalization and section identity. Per-operation wire payloads remain
in the vocabulary reference while that part is consolidated.

[lib.rs](src/lib.rs) is the codec entry map. The semantic module, proof bundle,
artifact envelope/manifest, debug map, installation payload, optimization
execution, obligation ledger, and observation profile have distinct owners.
Do not infer one section's identity from another section's current version.

Current markers live with their owners: semantic format in [lib.rs](src/lib.rs),
vocabulary in [VocabularyMarker](../../representations/terminal-psi/src/terminal_module/identity/vocabulary.rs),
proof format in [proof_bundle.rs](src/proof_bundle.rs), envelope in
[canonical_artifact.rs](src/canonical_artifact.rs), and manifest in
[artifact_manifest.rs](src/artifact_manifest.rs). The image-emission owner
maintains the separate [installation format](../../../omega/backend/images/image-emission/src/installation.rs).
Codec-owned [current-format tests](src/current_format_tests.rs) pin canonical
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

[terminal_trace_v1_profile.rs](src/terminal_trace_v1_profile.rs) implements
canonical profile encoding and module-bound acceptance for
[TerminalTraceV1](../../../../wiki/spec/terminal-psi/observations.md). Acceptance
validates the module, derives its complete identity and site roster independently,
and compares the decoded profile exactly.

The current encoder writes the terminal-external group with zero count; the
decoder rejects a nonzero count. That is an implementation fence, not an assertion
that the semantic trace excludes successful external termination. The
[canonical ProcessExit contract](../../../../wiki/spec/language/process_exit.md)
settles requirement-owned completion identity without a new source keyword.
The [source-to-verifier migration](../../../../TASKS.md#process-exit-contract)
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

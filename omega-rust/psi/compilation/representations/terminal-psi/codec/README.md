# Terminal codec

Public contract: [Terminal product](../../../../../../wiki/spec/terminal-psi/product.md).
The [canonical encoding contract](../../../../../../wiki/spec/terminal-psi/encoding.md)
owns canonicalization and section identity. Per-operation wire payloads remain
in the vocabulary reference while that part is consolidated.

[lib.rs](src/lib.rs) exposes the public API. Start with the operation's owner:

- [semantic_module.rs](src/semantic_module.rs) sequences canonical
  ordering, representation checks, encoding, decoding, and semantic identity.
  Its `*_wire` siblings own individual payloads; [foundation_validation.rs](src/foundation_validation.rs)
  checks representation shape and [wire_fields.rs](src/wire_fields.rs) owns byte cursors.
- [canonical_artifact.rs](src/canonical_artifact.rs) binds semantic,
  optimization, and optional debug bytes into the handoff envelope.
  Producer construction takes ownership of the module; external ingress
  decodes once and retains that storage. Views borrow it without fallible
  decoding or replay caches. The manifest is reconstructed from the sections.

The [Psi product contract](../../../../../../wiki/spec/terminal-psi/product.md#publication-and-separate-consumption)
permits checked judgments to be reused under host authorization of a trusted
producer's exact artifact. Canonical decoding establishes encoding and structural
validity, not truth of arbitrary claims or execution authority. No receiving
theorem rediscovery or certificate replay is required.

The codec carries certificate bytes in mathematical proof terms and indexed
theorem roots, including the `PSICORE` framing. Keep those canonical encodings;
they grant nothing at admission. The local proof kernel and interpreter's closed
evaluation uses remain separate from byte transport.

The [implementation plan](../../../../../../wiki/drafts/designs/psi_claim_admission.md)
splits canonical/structural inspection from execution-only preparation. Today
decode also performs module-foundation and dynamic-result analyses, including
descriptor and entailment work. The split preserves structural validation, moves
execution-only work to preparation and retains exact reusable results. Shared
input and aggregate work limits must cover these paths; per-query search caps
alone do not bound total loading cost to a host-granted budget.

The other [sections.rs](src/sections.rs) owners encode manifest, debug, optimization and
observation-profile data.
[lib.rs](src/lib.rs) owns the shared encoding error vocabulary.
Do not infer one section's identity from another section's current version.

Current markers live with their owners: semantic format in [semantic_module.rs](src/semantic_module.rs),
vocabulary in [VocabularyMarker](../src/identity.rs),
[canonical_artifact.rs](src/canonical_artifact.rs), and manifest in
[sections.rs](src/sections.rs). Installation section commitments remain distinct
from semantic identity and do not grant receiver-side installation authority.
The [module wire decoder](src/module_wire.rs) owns
incompatible-marker rejection. Source-lowering tests check semantic
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
does not establish its proofs. Source checking establishes the range facts;
receiving structural checks retain their exact correspondence, while source
judgments are reused only under the host's producer-trust authorization.

## Observation-profile implementation

[observation.rs](../src/observation.rs) defines the retained
profile for
[TerminalTraceV1](../../../../../../wiki/spec/terminal-psi/observations.md). Acceptance
validates the module, derives its complete identity and site roster independently,
and compares the decoded profile exactly. Revision 2
(`omega.terminal.observation-profile.v2`) appended the operation-crash group:
each trapping operation is its own `Trap` site rather than a fabricated edge
or boundary route. Its closed denotation retains either the integer primitive
and carriers or the non-finite float-result rule and IEEE format. Integer
denotation tags 1–8 keep their original bytes; tag 9 adds the float rule, so
an older decoder rejects it before consuming a carrier. Saturating float
adapters add no crash row. The original arithmetic boundary call remains a
separate operation with its own observation identity.

The current encoder writes the terminal-external group with zero count; the
decoder rejects a nonzero count. That is an implementation fence, not an assertion
that the semantic trace excludes successful external termination. The
[canonical ProcessExit contract](../../../../../../wiki/spec/language/process_exit.md)
settles requirement-owned completion identity without a new source keyword.
The [source-to-verifier migration](../../../../../../TASKS.md#process-exit-contract)
remains unimplemented; nonzero rows must continue to reject until that route
retains and independently verifies the exact terminal transfer.
Scalar and whole-root structural comparison helpers provide value comparison
only, not trace construction or refinement evidence.

## Current-only decoding

The module decoder accepts only the current semantic format and vocabulary
markers. It rejects every other marker before reading the module body, including
crossed format/vocabulary pairs. All current rosters are required; the decoder
does not supply missing historical rows or migrate an older artifact.
Proof, envelope and installation markers remain independently owned. Current
certificate-bearing forms remain carried and canonically decoded.

Run the codec's existing controls with:

```sh
cargo nextest run -p terminal-codec --lib --no-fail-fast --no-tests fail
```

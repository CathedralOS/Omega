# Product compiler source checkpoints

This directory publishes deterministic snapshots of the ordinary-Omega source
that implements the product compiler. A checkpoint is exact for the
functionality it claims; it does not freeze the final product file set or
`Ωself`.

Checkpoint artifacts remain separate:

- `checkpoint-NNNNNN.json` records compiled sources, virtual/compile-time
  sources, generator provenance, resolver-derived dependency edges, and
  content/closure digests;
- `profile-NNNNNN.md` records the compositional Omega facilities actually used
  by that closure and the current retain/refactor disposition. The compiler's
  `omega-source-snapshot --feature-census` mode supplies the exhaustive
  feature/resource observations;
- `profile-NNNNNN.json` binds the manifest digest, versioned feature catalog,
  provisional admission partition, resource ceilings, valid-Omega canaries,
  unresolved evidence, and its own domain-separated digest; and
- product and bootstrap task files record what functionality remains.

The manifest and census are evidence, never an allowlist for `omega-bootstrap`.
The bridge must implement the published profile generally and reject excluded
Omega before publication.

Current status: checkpoint 000001 passed at publication but is not a coherent
checkpoint of current `main`. The fast gate rejects compiled-source drift in
`compiler/psi/lex/lexer.omg`, `compiler/psi/tokens/tokens.omg`, and
`omega/language/std/console.omg`, plus provenance drift in `Cargo.lock` and
`bootstrap/onramps/omega-rust/omega/orchestration/omega-compiler/src/pipeline/stages.rs`.
The latter is also the provider of the pinned build-prelude snapshot, so its
drift must be reconciled with `inputs/build-prelude.omg` rather than treated as
an unrelated hash update. Refresh the manifest and profile as one product-owned
closure; do not weaken verification or update only the first mismatch. Its
pinned census remains bounded historical evidence for bridge cost work, not
current source-closure evidence.

Run `compiler/source-checkpoints/verify.sh` for the fast gate: it composes the
resolver-exact manifest/provenance gate with every-target profile census,
checked-Omega admission canaries, resource ceilings, and both manifest and
profile mutation teeth. It is part of `bootstrap/verify-lattice.sh`.

Run `compiler/source-checkpoints/checkpoint-000001.sh` for the complete product
checkpoint: it also reproduces generated Unicode source, compiles the hosted
native adapter, and runs the acceptance/rejection matrix. The explicit
`verify_manifest.py --content-only` mode deliberately skips resolver replay and
is never checkpoint acceptance.

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

Current status: checkpoint 000001 is a coherent snapshot of its refreshed
product closure. The fast gate pins compiled source and Cargo/provider
provenance, including the exact `BUILD_PRELUDE` extracted from
`source/compiler/rust/omega/orchestration/omega-compiler/src/pipeline/stages.rs`.
The refreshed snapshot includes the prelude's public build vocabulary and
package-identity declaration. Future drift must again refresh the manifest,
profile, and prelude together; verification must never be weakened or stopped
after the first mismatch. The hosted adapter publishes the versioned structural
`OMGLEX1` observation, and the complete gate compares it byte for byte with an
independent Rust encoder across accepted, rejected-prefix, and capacity cases,
then proves a tampered stream is rejected. The census remains bounded checkpoint
evidence for bridge cost work, not authority for later compiler phases.

Run `source/compiler/omega/source-checkpoints/verify.sh` for the fast gate: it composes the
resolver-exact manifest/provenance gate with every-target profile census,
checked-Omega admission canaries, resource ceilings, and both manifest and
profile mutation teeth. It is part of `bootstrap/verify-lattice.sh`.

Run `source/compiler/omega/source-checkpoints/checkpoint-000001.sh` for the complete product
checkpoint: it also reproduces generated Unicode source, compiles the hosted
native adapter, and runs the differential lexical-observation matrix. The explicit
`verify_manifest.py --content-only` mode deliberately skips resolver replay and
is never checkpoint acceptance.

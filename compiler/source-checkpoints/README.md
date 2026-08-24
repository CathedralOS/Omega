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
  feature/resource observations; and
- product and bootstrap task files record what functionality remains.

The manifest and census are evidence, never an allowlist for `omega-bootstrap`.
The bridge must implement the published profile generally and reject excluded
Omega before publication.

Run `compiler/source-checkpoints/verify.sh` for the fast gate: it replays native
source resolution for every declared target, compares the exact loaded-source,
alias, and import-edge closure, checks external/generator provenance, and runs
manifest mutation teeth. It is part of `bootstrap/verify-lattice.sh`.

Run `compiler/source-checkpoints/checkpoint-000001.sh` for the complete product
checkpoint: it also reproduces generated Unicode source, compiles the hosted
native adapter, and runs the acceptance/rejection matrix. The explicit
`verify_manifest.py --content-only` mode deliberately skips resolver replay and
is never checkpoint acceptance.

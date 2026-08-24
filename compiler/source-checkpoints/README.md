# Product compiler source checkpoints

This directory publishes deterministic snapshots of the ordinary-Omega source
that implements the product compiler. A checkpoint is exact for the
functionality it claims; it does not freeze the final product file set or
`Ωself`.

Checkpoint artifacts remain separate:

- `checkpoint-NNNNNN.json` records compiled sources, virtual/compile-time
  sources, generator inputs, exact dependency edges, and content digests;
- `profile-NNNNNN.md` records the compositional Omega facilities actually used
  by that closure and the current retain/refactor disposition; and
- product and bootstrap task files record what functionality remains.

The manifest is closure evidence, never an allowlist for `omega-bootstrap`.
The bridge must implement the published profile generally and reject excluded
Omega before publication.

Run `compiler/source-checkpoints/checkpoint-000001.sh` to verify the manifest,
reproduce generated Unicode source, compile the hosted native adapter, and run
the checkpoint's acceptance/rejection matrix. `verify_manifest.py` is also
available separately for the fast closure-only check.

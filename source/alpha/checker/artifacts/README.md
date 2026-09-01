# Persisted proof-checker artifact

`proof_checker_bytecode.tape` is the platform-independent Alpha tape for the
authoritative Gamma certificate checker.

Its construction lineage uses the accepted Gamma compiler:

```text
audited Alpha seed + canonical Gamma compiler tape
  (`../../../gamma/compiler/gamma_compiler_bytecode.tape`)
  -> `../implementations/gamma/check.gamma`
  -> `proof_checker_bytecode.tape`
```

`tests/proof-checker/reconstruct-artifact.sh` performs that construction once, compares the
result byte-for-byte with the committed artifact, stamps it into the audited
host seed, and exercises discriminating accept/reject controls. Repeating the
construction is an optional reproducibility diagnostic. The normal
checker and compiler-refinement gates consume this artifact through
`tools/bootstrap/proof-checker/artifact_env.sh`. The canonical Gamma compiler artifact may compile the same
source as differential evidence only; it is not this artifact's authority.

The current artifact is 271,096 bytes. It accepts the bounded `OMGCHK1` binary
frame documented in the parent README and constructs exact raw `source` and
`tape` indexed byte-tree constants internally. The rebuilt tape leaves 777,476
bytes in the 1,048,572-byte AlphaBootstrapV2 payload extent.

The checker source, persisted tape, and gate share the V2 frame and arena. The
complete gate admits a realistic maximum compiler tape with named lemmas and
normalization, admits the simultaneous outer maxima, and pins adjacent input,
subject, certificate, stack, and arena containment.

Regenerate deliberately with `tools/bootstrap/proof-checker/rebuild-artifact.sh`.
Commit a changed tape only with its
source or canonical-compiler change and a green reconstruction plus checker suite.

| Retained file | Role | Deletion condition |
| --- | --- | --- |
| `proof_checker_bytecode.tape` | The sole accepted platform-independent checker artifact. | Replace atomically with its authoritative source and green exact reconstruction. |

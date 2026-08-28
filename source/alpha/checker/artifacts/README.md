# Persisted proof-checker artifact

`check.tape` is the platform-independent Alpha tape for the authoritative Beta
certificate checker. Its construction lineage is deliberately below the
accepted Beta compiler:

```text
audited Alpha seed + Alpha-written assembler
  -> Beta cold compiler written in Alpha (`bc-alpha.alpha`)
  -> `implementations/beta/check.beta`
  -> `check.tape`
```

`../reconstruct-artifact.sh` performs that construction twice, compares both
results byte-for-byte with the committed artifact, stamps it into the audited
host seed, and exercises discriminating accept/reject controls. The normal
checker and compiler-refinement gates consume this artifact through
`../artifact_env.sh`. Compiling the same source with the accepted `bc.tape` is
useful differential evidence only; it is not this artifact's authority.

Regenerate deliberately with `rebuild.sh`. Commit a changed tape only with its
source or cold-compiler change and a green reconstruction plus checker suite.

# Persisted proof-checker artifact

`proof_checker_bytecode.tape` is the platform-independent Alpha tape for the
authoritative Beta certificate checker.

Its construction lineage is deliberately below the accepted Beta compiler:

```text
audited Alpha seed + Alpha-written assembler
  -> canonical Beta compiler written in Alpha (`../../../beta/compiler/beta_compiler.alpha`)
  -> `../implementations/beta/check.beta`
  -> `proof_checker_bytecode.tape`
```

`../reconstruct-artifact.sh` performs that construction once, compares the
result byte-for-byte with the committed artifact, stamps it into the audited
host seed, and exercises discriminating accept/reject controls. Repeating the
construction is an optional reproducibility diagnostic. The normal
checker and compiler-refinement gates consume this artifact through
`../artifact_env.sh`. The canonical Beta compiler artifact may compile the same
source as differential evidence only; it is not this artifact's authority.

The current artifact is 237,270 bytes. It accepts the bounded `OMGCHK1` binary
frame documented in the parent README and constructs exact raw `source` and
`tape` indexed byte-tree constants internally; the framing path leaves 24,870 bytes in
the 262,140-byte Alpha payload extent.

Regenerate deliberately with `rebuild.sh`. Commit a changed tape only with its
source or canonical-compiler change and a green reconstruction plus checker suite.

| Retained file | Role | Deletion condition |
| --- | --- | --- |
| `proof_checker_bytecode.tape` | The sole accepted platform-independent checker artifact. | Replace atomically with its authoritative source and green exact reconstruction. |
| `rebuild.sh` | Explicit mutation entry point for that artifact. | Delete when artifact replacement is owned by a different exact construction entry point. |

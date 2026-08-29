# Persisted Beta compiler artifact

`beta_compiler_bytecode.tape` is the platform-independent Alpha tape emitted
directly from the canonical Alpha-written Beta compiler.

Its construction lineage is exactly:

```text
audited Alpha seed + Alpha-written assembler
  -> beta_compiler.alpha
  -> beta_compiler_bytecode.tape
```

`cold-start/rebuild-artifact.sh --check` reconstructs that tape and compares it
byte-for-byte without changing the repository. `artifact_env.sh` stamps it
into the host's audited Alpha seed for downstream lattice gates. No Beta
self-host, textual Alpha output, or second assembler invocation lies on this
edge. The historical `bc.beta` implementation is a bounded differential subject
only and has no production consumer.

Regenerate deliberately with `cold-start/rebuild-artifact.sh`; commit a changed
tape only together with the source/compiler change and a green construction
check plus the directly relevant focused tests.

The committed artifact is 20,717 bytes. Its SHA-256 digest is
`e2b27ed9670fad3116d3cbcf41fe2a65d0da7ed681d3af2bd0aecb1785d10512`.
The reconstruction gate's byte comparison is authoritative for repository
identity; the digest is recorded for convenient audit and transport checks.

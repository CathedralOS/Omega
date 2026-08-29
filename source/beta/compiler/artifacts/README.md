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
edge.

Regenerate deliberately with `cold-start/rebuild-artifact.sh`; commit a changed
tape only together with the source/compiler change and a green construction
check plus the directly relevant focused tests.

The committed artifact is 20,977 bytes. Its SHA-256 digest is
`1911fc4f9667081ca96559ee970f07c3359f225c1177b5ed889d55c05a059f0f`.
The reconstruction gate's byte comparison is authoritative for repository
identity; the digest is recorded for convenient audit and transport checks.

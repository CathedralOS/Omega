# Persisted Beta compiler artifact

`bc.tape` is the platform-independent Alpha tape for the fixed-point Beta
compiler. D10 selects the descriptive destination name
`beta_compiler_bytecode.tape`. `bc.tape` remains the current committed path
until the atomic naming migration updates every consumer; the tape bytes and
content hash do not change merely because its path does.

Its only construction lineage is:

```text
Alpha seed + Alpha-written assembler
  -> cold-start/bc-alpha.alpha
  -> compile bc.beta once (initial bc)
  -> compile bc.beta again (persisted fixed-point bc.tape)
```

`cold-start/rebuild-artifact.sh --check` reconstructs that tape and compares it
byte-for-byte without changing the repository. `artifact_env.sh` stamps it
into the host's audited Alpha seed for downstream lattice gates. Fixed-point
repetition and the complete Beta corpus remain useful focused diagnostics, not
additional producer edges. The historical Rust producer is retired and was
never in this artifact's lineage.

Regenerate deliberately with `cold-start/rebuild-artifact.sh`; commit a changed
tape only together with the source/compiler change and a green construction
check plus the directly relevant focused tests.

The committed artifact is 40,693 bytes. Its SHA-256 digest is
`73a0087da97b0629617ba8ced637a7783b2cc6911be906d1b4df5801e65c2cdd`.
The reconstruction gate's byte comparison is authoritative for repository
identity; the digest is recorded for convenient audit and transport checks.

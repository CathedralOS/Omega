# Persisted Beta compiler artifact

`bc.tape` is the platform-independent Alpha tape for the fixed-point Beta
compiler. Its only construction lineage is:

```text
Alpha seed + Alpha-written assembler
  -> cold-start/bc-alpha.alpha
  -> compile bc.beta once (bootstrap compiler)
  -> compile bc.beta again (persisted fixed-point bc.tape)
```

`cold-start/full-source.sh` reconstructs that tape, compares it byte-for-byte,
checks another self-build generation, and runs the complete Beta corpus through
the persisted artifact. `artifact_env.sh` stamps it into the host's audited
Alpha seed for downstream bootstrap gates. The Rust producer is retained only
as diagnostic/on-ramp history and is not in this artifact's lineage.

Regenerate deliberately with `cold-start/rebuild-artifact.sh`; commit a changed
tape only together with the source/compiler change and a green full-source gate.

The committed artifact is 51,647 bytes. Its SHA-256 digest is
`e491073e0a27d50fb429d6f2a24158029ab5d8baa98e8855a4bd8771768c104b`.
The reconstruction gate's byte comparison is authoritative for repository
identity; the digest is recorded for convenient audit and transport checks.

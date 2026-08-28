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
Alpha seed for downstream bootstrap gates. The historical Rust producer is
retired and was never in this artifact's lineage.

Regenerate deliberately with `cold-start/rebuild-artifact.sh`; commit a changed
tape only together with the source/compiler change and a green full-source gate.

The committed artifact is 52,141 bytes. Its SHA-256 digest is
`1b32401c4c8fb60598e97178d415136227c9aa2231e28d9eb44b30e7a2818a2f`.
The reconstruction gate's byte comparison is authoritative for repository
identity; the digest is recorded for convenient audit and transport checks.

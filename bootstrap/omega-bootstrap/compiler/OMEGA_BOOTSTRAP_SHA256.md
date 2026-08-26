# Bounded structural SHA-256 producer

This contract defines one Delta-written SHA-256 producer used to check exact
bootstrap transport bytes. It is structural/digest consistency evidence only.
It is not an accepted-lock schema, a package-evidence artifact, a source-root
selection rule, or compilation authority.

The producer consumes raw bytes from standard input and, on success, emits the
32 raw SHA-256 digest bytes to standard output. It accepts input extents from
zero through 267,280 bytes, the public `OMGCOMP1`/`OMGCOMP2` framing ceiling.
An input byte at extent 267,281 selects checked exhaustion (`252`). No digest
byte may be published on exhaustion. Other host-I/O failure is outside this
sealed byte-I/O profile; every byte sequence within the declared extent has a
SHA-256 digest and therefore has no malformed-input (`251`) case.

SHA-256 is the FIPS 180-4 hash over the exact input byte sequence:

- append one `1` bit, then the minimum zero bits making the length congruent to
  448 modulo 512, then the original bit length as one 64-bit big-endian word;
- initialize the eight words to `6a09e667 bb67ae85 3c6ef372 a54ff53a
  510e527f 9b05688c 1f83d9ab 5be0cd19`;
- use the 64 standard SHA-256 round constants from `428a2f98` through
  `c67178f2`; and
- emit the final eight words in big-endian order.

The Delta implementation represents every 32-bit word as four independently
bounded bytes. Carries are propagated from the least-significant byte and the
carry beyond byte zero is discarded. Thus every modular-word arithmetic
intermediate stays below 1,280; the separate extent arithmetic peaks at the
2,138,240-bit public input ceiling. No result depends on signed-`i32` overflow,
ambient wrapping, or an unadmitted wide integer. Boolean functions, rotations,
and shifts are reconstructed byte/bit-wise with exact division and remainder.

## Evidence boundary

The focused gate must cover:

1. the fixed empty, `abc`, and 56-byte FIPS known-answer vectors;
2. independently calculated 55/56/63/64/65-byte padding boundaries;
3. the existing canonical two-unit `OMGCOMP1` envelope and its already pinned
   SHA-256 receipt;
4. input mutation changing the digest;
5. exact native and Delta-self-built output agreement; and
6. at least one Rust-free Delta-to-Gamma/Gamma known-answer observation.

The 267,281-byte adjacent exhaustion case must return `252` with empty output.
The gate does not call a digest accompanying an untrusted envelope authoritative:
the existing fixture receipt remains a drift pin. A future package-owned
accepted-closure projection must independently reconstruct the expected digest
before the equality described by `OMEGA_BOOTSTRAP_COMPILATION.md` can contribute
to compilation acceptance.

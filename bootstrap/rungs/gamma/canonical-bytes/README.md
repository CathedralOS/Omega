# Canonical byte primitives

This directory is the shared typed byte-cursor layer for low-rung canonical
decoders. Gamma currently has no module/import syntax, so consumers concatenate
`types.gamma` before their own data declarations and `decode.gamma` before their
own decoding functions.

The layer deliberately owns only:

- the immutable `Bytes` spine;
- generic integer and byte-tail success/failure cursors;
- checked byte reads;
- fixed-width little-endian `u16` and `u32` reads;
- exact little-endian `u64` reads as independently checked low/high `u32`
  halves, plus equality, unsigned ordering, and nonzero validation that never
  cross Gamma's signed `Int` range;
- exact little-endian `u128` reads as four independently checked low-to-high
  `u32` limbs, plus equality and zero validation; and
- exact-byte and zero-byte consumption.

It does not know the `PSITERM\0` marker, vocabulary versions, collection counts,
semantic identities, terminal tags, or any recursive terminal type. Reusable
canonical scalar/type/value/UTF-8 and structural-leaf grammar fragments are the
adjacent, independently gated `../terminal-codec-primitives/` responsibility.
That layer deliberately owns no fixed format/vocabulary header and is not a
complete live terminal codec. A future low-rung canonical decoder must preserve
the shared full-width `U64` carrier rather than laundering identities into a
signed host integer.

Run `sh bootstrap/rungs/gamma/test-canonical-bytes.sh` for the typed and independent
interpreter contract.

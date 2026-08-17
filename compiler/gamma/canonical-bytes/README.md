# Canonical byte primitives

This directory is the shared typed byte-cursor layer for low-rung canonical
decoders. Gamma currently has no module/import syntax, so consumers concatenate
`types.gamma` before their own data declarations and `decode.gamma` before their
own decoding functions.

The layer deliberately owns only:

- the immutable `Bytes` spine;
- generic integer and byte-tail success/failure cursors;
- checked byte reads;
- fixed-width little-endian `u16` and `u32` reads; and
- exact-byte and zero-byte consumption.

It does not know the `PSITERM\0` marker, vocabulary versions, collection counts,
semantic identities, strings, terminal tags, or any recursive terminal type.
The bounded ledger spike therefore continues to own its small-`u64` limitation
and every monomorphic type-specific parser result. The full canonical decoder
must resolve those honestly rather than laundering them into this primitive
layer.

Run `sh compiler/gamma/test-canonical-bytes.sh` for the typed and independent
interpreter contract.

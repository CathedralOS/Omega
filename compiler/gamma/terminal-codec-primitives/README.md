# Canonical terminal-codec primitives

This directory layers exact terminal-codec grammar over the semantics-neutral
`../canonical-bytes/` cursor. It owns the current envelope prefix:

- exact `PSITERM\0` magic;
- exact format marker 16; and
- exact current vocabulary marker 20, retained in the typed result.

It also owns the codec's length-prefixed UTF-8 string rule:

- a little-endian `u32` byte count;
- at most 1 MiB, matching `MAX_CONTENT_IDENTITY_BYTES`;
- the exact original bytes, including an empty string or embedded NUL; and
- canonical UTF-8 scalar encodings only—no overlong forms, surrogates, values
  above U+10FFFF, isolated continuation bytes, truncation, or normalization.

The scalar layer owns canonical Boolean bytes (`0` or `1`) and optional
semantic identities (`0` for absent, `1` plus one exact nonzero `u64` for
present). It retains the complete `U64`; a bounded consumer may narrow only in
an explicit adapter after decoding.

The semantic-identity layer owns required nonzero identities as a distinct
typed carrier over the complete `U64`, plus exact equality and canonical
unsigned ordering. Counts, tags, and byte offsets therefore cannot be confused
with identities by the full ledger decoder, and identities above Gamma's signed
`Int` range remain representable without narrowing.

The scalar-type layer owns the complete current type grammar: Boolean, fixed
signed or unsigned integers, and unsigned address integers, each with an exact
width in `1..=128`. The bounded spike now retains this exact type in every
declaration; only its operation-row policy remains intentionally limited to
Boolean, signed i8, and signed i16.

The v16 envelope also admits format-annotated IEEE structural leaves and their
atomic equality/inequality comparison proposition. They are outside this scalar-type primitive layer
and the bounded Gamma ledger fixtures; those decoders continue to reject the new
tags if they appear instead of silently assigning them scalar meaning.

The integer-value layer owns the complete current payload grammar: tag `1`
retains one signed value's exact 128-bit two's-complement bits and tag `2`
retains one unsigned value's exact 128-bit bits. It does not narrow either form
to Gamma `Int`; it owns exact signed/unsigned payload equality, and bounded
consumers must validate and narrow explicitly after the shared decoder succeeds.

Each result retains its unread input tail; strings additionally retain a
separate captured byte spine. The module does not assign semantic meaning to an
identity, path, or label. Run
`sh compiler/gamma/test-terminal-codec-primitives.sh` for the typed and
independent-interpreter contract.

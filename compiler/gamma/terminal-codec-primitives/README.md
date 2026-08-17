# Canonical terminal-codec primitives

This directory layers exact terminal-codec grammar over the semantics-neutral
`../canonical-bytes/` cursor. It owns the current envelope prefix:

- exact `PSITERM\0` magic;
- exact format marker 11; and
- exact current vocabulary marker 16, retained in the typed result.

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

Each result retains its unread input tail; strings additionally retain a
separate captured byte spine. The module does not assign semantic meaning to an
identity, path, or label. Run
`sh compiler/gamma/test-terminal-codec-primitives.sh` for the typed and
independent-interpreter contract.

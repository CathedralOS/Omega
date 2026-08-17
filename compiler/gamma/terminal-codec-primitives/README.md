# Canonical terminal-codec primitives

This directory layers exact terminal-codec grammar over the semantics-neutral
`../canonical-bytes/` cursor. It currently owns the codec's length-prefixed
UTF-8 string rule:

- a little-endian `u32` byte count;
- at most 1 MiB, matching `MAX_CONTENT_IDENTITY_BYTES`;
- the exact original bytes, including an empty string or embedded NUL; and
- canonical UTF-8 scalar encodings only—no overlong forms, surrogates, values
  above U+10FFFF, isolated continuation bytes, truncation, or normalization.

The result retains a separate captured byte spine and unread input tail. It
does not assign semantic meaning to an identity, path, or label. Run
`sh compiler/gamma/test-terminal-codec-primitives.sh` for the typed and
independent-interpreter contract.

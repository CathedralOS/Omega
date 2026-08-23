# Canonical terminal-codec primitives

This directory layers exact terminal-codec grammar over the semantics-neutral
`../canonical-bytes/` cursor. It owns the frozen envelope prefix used by the
bounded terminal-ledger feasibility experiment:

- exact `PSITERM\0` magic;
- exact format marker 18; and
- exact vocabulary marker 20, retained in the typed result.

These are not the current deployment markers (now format 22/vocabulary 25).
This directory is a historical artifact-assurance experiment, not an alternate
terminal codec and not part of Gamma language meaning.

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

The separately gated structural-leaf layer owns the exact v18 byte grammar for:

- IEEE binary32/binary64 format and equality/inequality comparison kinds;
- borrowed-view and exact-full-width-capacity bounded-owned byte-sequence
  carriers;
- canonical structural fields with one full-width nonzero root identity and an
  ordered path of full-width nonzero field/case identities or full-width fixed
  indices; case segments select one sum case before its exact payload field; and
- atomic IEEE comparison, byte-sequence equality, and structural
  case-membership proposition tags `11`, `12`, and `13`. Case membership
  deliberately permits an empty subject path for a whole structural root.

It enforces nonempty IEEE/byte-sequence leaf paths and their exact canonical
operand order: root first, then lexicographic path, with field segments before
fixed-index segments before case segments and every numeric component ordered
as unsigned `u64`.
The layer retains the unread input tail and rejects invalid tags, zero semantic
identities, reversed operands, and truncation. It assigns no declared type or
runtime meaning to a decoded field. The bounded Gamma ledger fixtures do not
concatenate this new layer and continue to reject IEEE and byte-sequence tags
rather than silently widening their claimed semantic coverage.

The integer-value layer owns the complete current payload grammar: tag `1`
retains one signed value's exact 128-bit two's-complement bits and tag `2`
retains one unsigned value's exact 128-bit bits. It does not narrow either form
to Gamma `Int`; it owns exact signed/unsigned payload equality, and bounded
consumers must validate and narrow explicitly after the shared decoder succeeds.

Each result retains its unread input tail; strings additionally retain a
separate captured byte spine. The module does not assign semantic meaning to an
identity, path, or label. `structural_leaves_types.gamma` and
`structural_leaves.gamma` are kept separate so bounded consumers need not import
unsupported proposition vocabulary merely to reuse the scalar primitives. Run
`sh bootstrap/rungs/gamma/test-terminal-codec-primitives.sh` for the typed and
independent-interpreter contract.

# wire_canonical_varint

First sample exercising canonical-LEB128 enforcement: the
generated `Scalar::decode` rejects an overlong zero group
and payload bits beyond u64 (both return
`WireVerdict::Invalid`), while the canonical ten-group
`u64::MAX` frame decodes `Sound`.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.

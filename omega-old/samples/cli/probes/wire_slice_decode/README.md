# wire_slice_decode

First sample decoding a borrowed `&[u8]` wire member:
`#0 bytes: &[u8]` synthesizes `Blob::encode`/`Blob::decode`;
decode stores a `{ptr, len}` descriptor viewing the decode
buffer in place (zero-copy, no allocator) and reports
through a `WireVerdict` sum whose `Invalid` case is ZII.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.

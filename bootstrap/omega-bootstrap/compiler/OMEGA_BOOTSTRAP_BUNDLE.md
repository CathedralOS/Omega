# Canonical bridge source bundle, version 1

The bridge consumes one deterministic byte stream. The `OMG0BNDL` magic and
`.omg0b` extension are retained legacy version-1 wire identifiers; they do not
name `Omega0`, a compiler generation, or a language rung. New architecture and
task prose call this artifact the **bridge source bundle**. Until native
package loading is implemented, multiple source files use this canonical
length-delimited bundle.
The bundle is an auditable compiler input artifact: paths and source bytes are
preserved exactly, and malformed or noncanonical encodings reject.

## Version 1 wire format

All integers are unsigned little-endian. Counts and lengths must be no greater
than `2^31 - 1`, so a D0 implementation can validate and represent them with its
checked `i32` arithmetic.

```text
8 bytes   magic: ASCII "OMG0BNDL"
u32       version: 1
u32       source count (at least 1)

repeated source count times:
  u32     label byte length (at least 1)
  u32     content byte length
  bytes   ASCII label
  bytes   exact source content
```

Labels are strictly increasing by raw byte order and unique. A label is a
relative POSIX path composed only of ASCII letters, digits, `.`, `_`, `-`, and
`/`. It may not begin or end with `/`; contain an empty, `.` or `..` component;
or contain `\\`. The decoder rejects trailing bytes after the final entry.

Content is opaque: it may be empty, omit a final newline, or contain any byte.
No separator is injected, so two tokens from adjacent files cannot fuse and
diagnostics can retain the exact label and byte offset. The bridge frontend is
responsible for accepting UTF-8 source after decoding the bundle.

## Tool and gate

`omega_bootstrap_bundle.py` is an untrusted pack/inspection convenience, not
part of the language or trust base:

```sh
python3 bootstrap/omega-bootstrap/compiler/omega_bootstrap_bundle.py pack \
  src/main.omg=src/main.omg lib/math.omg=lib/math.omg > sources.omg0b
python3 bootstrap/omega-bootstrap/compiler/omega_bootstrap_bundle.py verify sources.omg0b
python3 bootstrap/omega-bootstrap/compiler/omega_bootstrap_bundle.py manifest sources.omg0b
python3 bootstrap/omega-bootstrap/compiler/omega_bootstrap_bundle.py get sources.omg0b src/main.omg
```

The gate proves invocation-order independence, exact-byte round trips (including
NUL and missing final newline), canonical manifest order, and rejection of
duplicates, unsafe labels, truncation, and trailing data. The Delta-written
decoder canary is
[`../../rungs/delta/samples/omega-bootstrap-bundle-decode.alp`](../../rungs/delta/samples/omega-bootstrap-bundle-decode.alp).
It streams the format with checked `i32` lengths, canonical path/order checks,
exact EOF, and an explicit local label-storage exhaustion result at 64 label
bytes in this first canary. Native and Rust-free meaning gates cover canonical
input, truncation, trailing bytes,
noncanonical ordering, and the decoder's explicit resource ceiling; the native
gate also exercises an unsafe path label.

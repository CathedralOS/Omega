# Descriptor access stops before padding

On macOS ARM64, from this directory:

```sh
omega run --both main.omg < input.txt
```

Both engines must exit 0 and print `CA` followed by LF. A helper lends the
original nested record to Darwin `read`. The first one-byte read changes only
the low byte of a two-byte header; its high byte remains `A`. Later reads share
the same finite cursor and preserve backing on zero-count and EOF results.
Readonly writes observe the original updated header. The record's neighboring
fields and its containing owner's prefix/suffix stay unchanged.

The native record has interior and trailing padding, but neither read nor write
accesses it. The runtime must demand scalar coverage for the actual byte prefix,
not for the whole object. A partial scalar retains its original unread bytes
before staged decode and authenticated writeback. Accessed padding remains
unavailable without actual byte backing; this fixture does not license its
contents or pointer/atomic storage. No external package is needed.

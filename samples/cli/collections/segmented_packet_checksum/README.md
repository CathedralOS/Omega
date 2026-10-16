# Segmented packet checksum

A packet holds four borrowed segments as `Missing` or `Bytes` cases. A rolling
base-31 checksum consumes their bytes in order without copying or flattening the
backing storage. Nested state loops visit the segments and their live windows.

The authored oracle is exit 0: windows into bytes 1, 2, 3, 4, 5 yield 986115;
missing and empty segments add nothing, an empty packet yields zero, a second
read yields the same result, and the sequence and adjacent sentinel bytes remain
unchanged. Arithmetic is explicitly modulo 2^32, not cryptographic integrity.

Run from the repository root:

```sh
omega run --both samples/cli/collections/segmented_packet_checksum/main.omg
```

The complete sample exits 0 natively and in Terminal execution. It exercises
typed case-path retirement, descriptor-aware residual cleanup and recursive zero
initialization without flattening the packet or copying its borrowed bytes.

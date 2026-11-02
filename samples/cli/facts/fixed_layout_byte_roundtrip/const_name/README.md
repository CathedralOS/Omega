A byte roundtrip through an explicit fixed layout lends a decoded view only after its payload bound is established, preserving every field and neighboring storage.

Complete initialized source representations cross ordinary copy-eligible constants; runtime copies materialize their backing before the unchanged byte roundtrip and guarded decode. Constants lend no runtime address.

The 40-byte sparse layout, bound42, fields, original arrays, canaries and
payload42/41 → result1/0 oracle are unchanged. The twin removes only the decoder
guard. [Run 41436633328](https://github.com/CathedralOS/Omega/actions/runs/41436633328)
qualifies the pair and ordinary sample on Linux x86_64 with optimized producer
`e1c62c271d9e`: native and Terminal agree at exit 0, and the guard-only twin
rejects. See the [parent fact](../README.md) for recorded epochs and remaining
hosted/product acceptance.

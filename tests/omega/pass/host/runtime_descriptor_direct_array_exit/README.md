On macOS AArch64, review this project's exact foreign supplies and run:

```sh
cd tests/omega/pass/host/runtime_descriptor_direct_array_exit
omega update
# Review the generated decisions, then: omega update --resume
omega run --both main.omg < /dev/null
```

Both engines return zero. Each independently produces `dir\nir\n` on stdout
and `p\n` on stderr; the corpus checks both stream oracles. The CLI currently
compares exit presentation only.

The whole fixed array and its nonzero-offset slice supply their original
pointer and exact length to the same foreign byte-view ABI. A projected
readonly scalar loan exposes only its four-byte field; writing two bytes must
leave the source field, including the remaining bytes, unchanged.

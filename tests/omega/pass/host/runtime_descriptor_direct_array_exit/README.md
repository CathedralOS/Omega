The Linux and Darwin entries call concrete native boundary traits with
`SystemVByteViewCalling` and `DarwinArm64ByteViewCalling`, respectively.
Package-local `Build::bind` rows attach the reviewed native `write` symbol to
each exact requirement. The entry bodies preserve the same write and backing
oracle for both calling conventions.

The complete array and its nonzero-offset slice lend their original pointer and
exact length. A projected readonly scalar loan lends its four-byte field; the
two-byte write preserves that field, including the remaining bytes.

Expected output from each engine: `dir\nir\n` on stdout and `p\n` on stderr.

Prepare and review this project's native supplies, then run on Linux x86-64 or
macOS ARM64:

```sh
omega update --project .
omega run --both main.omg < /dev/null
```

Expected native and Terminal exit: 0. The corpus checks stdout and stderr
independently for both engines; the CLI compares exit presentation.

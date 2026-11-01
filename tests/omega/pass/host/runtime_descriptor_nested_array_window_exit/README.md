The Linux and Darwin entries call concrete native boundary traits with
`SystemVByteViewCalling` and `DarwinArm64ByteViewCalling`, respectively.
Package-local `Build::bind` rows attach the reviewed native `write` symbol to
each exact requirement. The entry bodies preserve the same write and backing
oracle for both calling conventions.

A helper lends both a nonempty window and an empty window of nested
caller-owned array backing. The writes preserve the enclosing record and bytes
outside each window.

Expected output from each engine: `ir\n` on stdout.

Prepare and review this project's native supplies, then run on Linux x86-64 or
macOS ARM64:

```sh
omega update --project .
omega run --both main.omg < /dev/null
```

Expected native and Terminal exit: 0. The corpus checks stdout and stderr
independently for both engines; the CLI compares exit presentation.

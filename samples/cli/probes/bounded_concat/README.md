# Bounded byte copies

The [probe](main.omg) copies the five bytes of `"Room "` into a 16-byte array at
offset 0. It then copies the label's two live bytes, `"A1"`, from its four-byte
array into that same destination at offset 5. The destination stores its used
length separately. The entry checks that the live view equals `"Room A1"` and
has length 7, then traps if either check fails.

The [Build file](build.omg) selects an entry for all four hosted targets. The
probe currently stops before the entry executes. On macOS arm64 at
`93f7c2a52bf`, checking refused the fixed-array ASCII predicates and the
data-attached `Main::main` declaration. [The repair queue](../../../../OWNER_CODE_CONCERNS.md#the-bounded-copy-probe-stops-before-its-byte-and-length-checks)
tracks the remaining compiler and source work.

Reproduce from this directory:

```sh
omega --check --build-input build.omg --build-input main.omg ./main.omg < /dev/null
omega run --both --build-input build.omg --build-input main.omg ./main.omg < /dev/null
```

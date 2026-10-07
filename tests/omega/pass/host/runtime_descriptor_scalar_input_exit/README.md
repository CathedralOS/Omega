Run on macOS AArch64 after reviewing this project's exact foreign supplies:

```sh
cd tests/omega/pass/host/runtime_descriptor_scalar_input_exit
omega update
# Review pending decisions, then: omega update --resume
omega run --both main.omg < input.txt
```

Both engines return zero and output the authored `abc\n` bytes. The first
read replaces only three bytes of a caller-owned word; the other read consumes
the one remaining byte into a different field. Their untouched high bytes
remain unchanged. Zero-count and EOF reads consume or replace no bytes. The
import slots must share one invocation's finite input cursor.

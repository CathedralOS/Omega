Run on macOS AArch64 or Linux x86-64/AArch64 after reviewing this project's
exact foreign supplies:

```sh
cd tests/omega/pass/host/runtime_descriptor_scalar_input_exit
omega update
# Review pending decisions, then: omega update --resume
omega run --both main.omg < input.txt
```

Both engines return zero and output the authored `abc\n` bytes on each stream.
Discarding a write count must still perform its transfer; the independent stderr
oracle catches skipped Unit-result calls. The first
read replaces only three bytes of a caller-owned word; the other read consumes
the one remaining byte into a different field. Their untouched high bytes
remain unchanged. Zero-count and EOF reads consume or replace no bytes. The
import slots must share one invocation's finite input cursor.

Darwin uses exact libSystem imports; Linux uses the target's exact read/write
syscalls. They share the same original-loan capture/writeback protocol. Linux
cross-target Terminal execution does not qualify native Linux execution.

# Borrowed stream window

Run `mbx xtask sample-gate --filter samples/cli/probes/borrowed_stream_window`.
The authored result is exit 0: requests of 3 and 9 bytes produce windows of
3 and 4 bytes from an eight-byte buffer while retaining a stream borrow.

The bounds proof observes count and buffer length, not the unrelated stream
identity. That identity remains an evaluated operand; it is not required to
have an arithmetic representation before an independent bound can be proved.
The bounds contract stays intact. Native and Terminal execution have been
observed on macOS ARM64; the other bound targets still need matching-host runs.

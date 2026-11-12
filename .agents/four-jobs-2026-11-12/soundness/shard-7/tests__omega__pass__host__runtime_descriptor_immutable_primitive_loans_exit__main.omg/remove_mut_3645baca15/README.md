# Loans from primitive value bindings

On macOS arm64, run from the repository root:

```sh
omega run --both tests/omega/pass/host/runtime_descriptor_immutable_primitive_loans_exit/main.omg < tests/omega/pass/host/runtime_descriptor_immutable_primitive_loans_exit/input.txt
```

Ordinary `let` bindings own their contents. Their lack of binding-replacement
permission must not prevent a helper or exact host import from borrowing those
contents. Boolean writes and forwarded floating-point input retain the original
home; later observations read that home rather than a stale initializer.
Captured exclusive references keep that same home. A mutated integer index
selects the current array element through the ordinary guarded read path.
Separately copied values remain snapshots. Empty and EOF reads do not modify
the value. Output is four authored input bytes followed by the canonical true
Boolean byte and a newline, not a textual rendering of either scalar.

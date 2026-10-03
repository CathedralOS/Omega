# Erased record initializers

`omega run --both tests/omega/pass/drops/erased_record_initializer_exit/main.omg`
constructs a record whose erased proof field sits between two runtime fields.
The authored initializer order differs from declaration order. A helper borrows
the completed record and observes both runtime values; native and interpreter
execution exit 70. The Evidence initializer and cleanup have no runtime storage.

Prepare packages with `omega update --project tests/omega/pass/drops/erased_record_initializer_exit --target macos_arm64` on macOS, substituting the host target elsewhere.

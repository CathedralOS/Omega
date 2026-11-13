Nested cases, records, default array fields, owned forwarding and array literals use the same constructed-argument schedule in Unit and scalar callers. Expected exit: 70.

Run on macOS ARM64 through the public compiler:

```sh
omega update --project tests/omega/pass/calls/runtime_nested_case_argument_exit --target macos_arm64 --offline
omega run --both tests/omega/pass/calls/runtime_nested_case_argument_exit/main.omg
```

If update produces a package review, record its decisions and resume:

```sh
omega update --resume --project tests/omega/pass/calls/runtime_nested_case_argument_exit --offline
```

Refresh the accepted package lock when the compiler or package changes.

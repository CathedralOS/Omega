# Copied generic case-dispatch subjects

From the repository root, prepare and run the authored program:

```sh
mbx run -p omega -- update --project tests/omega/pass/generics/runtime_copied_indexed_case_subject_exit --target macos_arm64
mbx run -p omega -- run --both tests/omega/pass/generics/runtime_copied_indexed_case_subject_exit/main.omg
```

Both engines must exit with 70. `FixedBuffer<2>` and `FixedBuffer<3>` use
independent copies of the attached machine. Each effectful selector must run
once, and its saved subject must supply both payload reads: the two calls
produce 30 and 40, while each instance records one selection.

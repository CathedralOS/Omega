# Task lifecycle custody

This customer lends a linear `Task<i32>` to the generic `TaskLifecycle::request_cancel`
requirement, then transfers it through an
ordinary helper to the `TaskLifecycle::finish` boundary requirement and disposes its
affine outcome. Borrowing retains the claim; finishing consumes it once.

Run the public product route with:

```sh
omega run --both tests/omega/pass/core/task_lifecycle_operations/main.omg
```

The source/Terminal regression gate is:

```sh
python3 tools/corpus_gate.py --filter core/task_lifecycle_operations
```

The current frontier is recorded in
`TASKS.md` under `STATIC-LOWERING-BORROWED-ARGUMENT-TRANSFER`. This fixture
does not supply a runtime implementation of `TaskLifecycle::finish`; source custody
and Terminal production do not establish executable provider coverage.

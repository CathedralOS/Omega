# Task lifecycle custody

This customer borrows a linear `Task<i32>`, then transfers it through an
ordinary helper to the `Task::finish` boundary requirement and disposes its
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
does not supply a runtime implementation of `Task::finish`; source custody
and Terminal production do not establish executable provider coverage.

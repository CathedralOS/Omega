# drop_ordered_body

A non-empty `Guard::drop` body: source-ordered calls to attached helpers (`Teardown::unlock(); Teardown::flush();`) run at the cleanup edge when an owned `Guard` local dies at end of state.

## Learnings

- The reserved `Guard::drop(guard: &mut Guard)` signature takes the dying owner by `&mut`.
- An owned local that stops being live at end of state triggers the selected hook without an explicit `drop()` call; this probe also verifies implicit end-of-scope cleanup, unlike `drop_erased_member` (explicit `drop()` + erased members).

Compile-checked only: run a check target, not native.

# Unchanged mutable selector restoration

A mutable selector retains its value while an affine element is extracted and
its original place repaired. Only afterward does the selector change. Calls
with indices zero and one distinguish caller-visible replacement from the
subsequent occupied store. A call writing unrelated storage between extraction
and repair must not invalidate the selector. A second transform saves and copies
the selector, changes its former storage through a call, and repairs using the
detached copy. Correct execution returns 0.

```text
omega update --project tests/omega/pass/ownership/runtime_mutable_selector_window_exit
omega run --both tests/omega/pass/ownership/runtime_mutable_selector_window_exit/main.omg
```

# Computed snapshot restoration

An immutable computed selector captures its value before the original mutable
selector changes. A constant shift of that captured value selects and restores
the same element. Anonymous and suffixed constant arithmetic agree on element
one for replacement, leaving element zero unchanged. Correct execution
returns 0.

```text
omega update --project tests/omega/pass/ownership/runtime_computed_selector_snapshot_window_exit
omega run --both tests/omega/pass/ownership/runtime_computed_selector_snapshot_window_exit/main.omg
```

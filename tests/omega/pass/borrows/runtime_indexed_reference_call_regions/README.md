Run from the repository root:

```text
omega run --both tests/omega/pass/borrows/runtime_indexed_reference_call_regions/main.omg
```

Both engines return `0`. The caller asserts selected and neighboring values from its original storage. Runtime selectors remain in their authored regions around stored-reference pointer crossings.

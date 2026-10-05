# Stored reference leaf retirement

From the repository root, after the ordinary package update review:

```sh
omega run --both tests/omega/pass/borrows/runtime_stored_reference_leaf_call_retirement_exit/main.omg
```

The program reads a record's borrowed payload, writes 81 to the released backing,
then observes the record's still-live scalar sibling. Both execution engines
must exit 70. Releasing the reference leaf must preserve sibling storage and
must not invent a borrowed-storage repair window.

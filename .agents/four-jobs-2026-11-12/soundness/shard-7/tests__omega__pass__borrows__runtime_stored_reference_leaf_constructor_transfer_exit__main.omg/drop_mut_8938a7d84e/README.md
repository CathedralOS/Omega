# Partial reference transfer

From the repository root, prepare the fixture with `omega update --project
tests/omega/pass/borrows/runtime_stored_reference_leaf_constructor_transfer_exit`,
reviewing local package changes if requested. Then run:

```text
omega run --both tests/omega/pass/borrows/runtime_stored_reference_leaf_constructor_transfer_exit/main.omg
```

Both engines must return `70`. Moving the mutable-reference leaf into another
record must preserve the first record's scalar sibling. Retiring the transferred
loan permits mutation of the original owner without consuming either record's
remaining scalar storage.

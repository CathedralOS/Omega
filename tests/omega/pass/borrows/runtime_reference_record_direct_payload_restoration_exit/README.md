# Stored record-reference observations

From the repository root:

```sh
omega run --both tests/omega/pass/borrows/runtime_reference_record_direct_payload_restoration_exit/main.omg
```

The program reads a bounded integer and Boolean through a stored mutable
reference, reads its scalar sibling, then writes the backing cell. Native and
checked-interpreter execution both exit 70. Package inputs must have been
accepted through the ordinary `omega update` review first.

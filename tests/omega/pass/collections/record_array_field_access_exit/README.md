# Indexed record fields

From the repository root:

```sh
omega run --both tests/omega/pass/collections/record_array_field_access_exit/main.omg
```

The authored program replaces two reference-bearing array elements, then reads
their surrounding record fields and the first element's borrowed command.
It must exit 0 and print the three lines in `expected_stdout.txt`; native and
checked-interpreter execution must agree. The opaque console binding is a
disjoint sibling, not permission to discard the room's write footprint or to
claim knowledge of the binding's contents.

The current compiler frontier belongs to
`STATE-GRAPH-SCALAR-RESULT-CUSTOMERS` in `TASKS.md`.

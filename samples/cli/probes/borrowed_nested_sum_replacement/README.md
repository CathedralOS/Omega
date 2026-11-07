# Borrowed nested sum replacement

Directly replace an exclusive loan's two-level sum, cleaning the active old
token exactly once. Exercise an absent outer payload, an empty inner payload,
and a populated inner payload. Inner-tag inspection is valid only after the
outer `Present` case is selected. Disjoint markers remain unchanged. Correct
execution returns 0; wrong observations trap.

From the repository root, with the compiler available as `omega`:

```text
omega update --project samples/cli/basics/borrowed_nested_sum_replacement
omega run --both samples/cli/basics/borrowed_nested_sum_replacement/main.omg
```

This program exercises **BORROWED-STORAGE-RESTORATION**, not an
expected-rejection fixture. Keep the
direct assignment: explicitly extracting the old value would bypass the
runtime-selected cleanup capability being tested.

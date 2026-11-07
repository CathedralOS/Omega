# Borrowed sum population

Replace an exclusive loan's sum directly, letting assignment clean up its old
value. The caller begins with both empty and populated sums; the repaired
values must produce 42, retain disjoint markers and consume each sibling once.
Correct execution returns 0; wrong observations trap.

From the repository root, with the compiler available as `omega`:

```text
omega update --project samples/cli/basics/borrowed_sum_population
omega run --both samples/cli/basics/borrowed_sum_population/main.omg
```

The old sum's tag is unknown, so assignment selects its nominal payload cleanup
at runtime before storing the replacement. This exercises
**BORROWED-STORAGE-RESTORATION**, not an expected-rejection fixture. Keep the
direct assignment: manually extracting and dropping the old value would bypass
the operation this program demonstrates.

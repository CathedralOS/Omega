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

Current frontier: `populate` cannot publish its statement cleanup actions.
The old sum's tag is unknown, so its payload's nominal cleanup must be selected
at runtime. The cleanup action planner currently requires a statically selected
case. **BORROWED-STORAGE-RESTORATION** owns this remaining replacement path.
The sample is an unfinished valid program, not an expected-rejection fixture;
keep the direct assignment rather than manually extracting and dropping its
old value to bypass the missing operation.
